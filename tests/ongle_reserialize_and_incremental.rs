//! 편집한 구역을 다시 쓸 때의 손실 두 가지와 증분 조판의 동치성 (Ongle QA 2026-10-17 §7).
//!
//! ## 1. HWP3 변환본: 저장할 때마다 vpos 가 줄어 쪽 나눔이 움직인다
//!
//! 파서는 HWP3→HWP5 변환본을 열 때 문단 위/아래 간격을 반으로 줄이고 본문 LINE_SEG
//! vertical_pos 에서 누적 문단 위 간격을 뺀다(`parser/mod.rs`). 편집 없는 저장은 원본
//! 스트림을 그대로 쓰므로 무사하지만, 편집한 구역·새 문단 모양은 보정된 값이 그대로 파일에
//! 실려 다시 열 때 보정이 한 번 더 걸렸다(`HWP5-nopassword-123456.hwp`: 저장마다 −852HU,
//! 5쪽 끝 한 줄이 6쪽에서 5쪽으로). 저장기가 파일 단위로 되돌린다.
//!
//! ## 2. 제목 차례 표시(0x08)가 사라지고 컨트롤이 움직인다
//!
//! PARA_TEXT 인라인 제어 문자 0x08(`titm`) 은 IR 객체가 없어 버려졌고, `char_offsets` 에
//! 8 유닛 구멍만 남았다. HWP5 저장기는 구멍을 다음 컨트롤로 메우거나 버렸고, HWPX 저장기는
//! 슬롯 수가 맞지 않아 같은 문단의 책갈피·그림을 텍스트 뒤로 몰아냈다.
#![cfg(not(target_arch = "wasm32"))]

use rhwp::wasm_api::HwpDocument;

fn sample(name: &str) -> Vec<u8> {
    std::fs::read(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("samples")
            .join(name),
    )
    .unwrap()
}

/// 모든 구역을 "편집된 구역"처럼 다시 써서 연다.
fn reserialize(doc: &mut HwpDocument) -> HwpDocument {
    for s in &mut doc.document_mut().sections {
        s.raw_stream = None;
    }
    HwpDocument::from_bytes(&doc.export_hwp_native().unwrap()).unwrap()
}

#[test]
fn hwp3_variant_vpos_survives_repeated_saves() {
    let mut doc = HwpDocument::from_bytes(&sample("HWP5-nopassword-123456.hwp")).unwrap();
    assert!(doc.document().is_hwp3_variant);
    let vpos = |d: &HwpDocument| -> Vec<i32> {
        d.document().sections[0]
            .paragraphs
            .iter()
            .flat_map(|p| p.line_segs.iter().map(|l| l.vertical_pos))
            .collect()
    };
    let before = vpos(&doc);
    let mut again = reserialize(&mut doc);
    assert_eq!(vpos(&again), before, "1회 저장");
    let third = reserialize(&mut again);
    assert_eq!(vpos(&third), before, "2회 저장");
}

#[test]
fn hwp3_variant_new_para_shape_keeps_spacing() {
    let mut doc = HwpDocument::from_bytes(&sample("HWP5-nopassword-123456.hwp")).unwrap();
    let pi = doc.document().sections[0]
        .paragraphs
        .iter()
        .position(|p| p.para_shape_id == 4)
        .unwrap();
    doc.apply_para_format_native(0, pi, r#"{"alignment":"center"}"#)
        .unwrap();
    let id = doc.document().sections[0].paragraphs[pi].para_shape_id as usize;
    let want = {
        let ps = &doc.document().doc_info.para_shapes[id];
        (ps.spacing_before, ps.spacing_after)
    };
    let back = HwpDocument::from_bytes(&doc.export_hwp_native().unwrap()).unwrap();
    let ps = &back.document().doc_info.para_shapes[id];
    assert_eq!((ps.spacing_before, ps.spacing_after), want);
}

#[test]
fn title_marks_and_control_slots_survive_section_rewrite() {
    let mut doc = HwpDocument::from_bytes(&sample("HWP5-nopassword-123456.hwp")).unwrap();
    let marks: usize = doc.document().sections[0]
        .paragraphs
        .iter()
        .map(|p| p.inline_ctrl_chars.len())
        .sum();
    assert!(marks > 0, "샘플에 제목 차례 표시가 있어야 한다");
    let shape = |d: &HwpDocument| -> Vec<(String, Vec<u32>, usize)> {
        d.document().sections[0]
            .paragraphs
            .iter()
            .map(|p| {
                (
                    p.text.clone(),
                    p.char_offsets.clone(),
                    p.inline_ctrl_chars.len(),
                )
            })
            .collect()
    };
    let before = shape(&doc);
    let back = reserialize(&mut doc);
    assert_eq!(shape(&back), before);
}

#[test]
fn hwpx_keeps_controls_before_title_mark_hole() {
    // 문단 68: [책갈피][그림] 제목 텍스트 [제목 차례 표시]. 종전엔 책갈피·그림이 텍스트 뒤로 갔다.
    let doc = HwpDocument::from_bytes(&sample("HWP5-nopassword-123456.hwp")).unwrap();
    let back = HwpDocument::from_bytes(&doc.export_hwpx_native().unwrap()).unwrap();
    let p = &back.document().sections[0].paragraphs[68];
    assert!(p.text.contains("주의할 점"));
    assert!(
        p.char_offsets.first().copied().unwrap_or(0) >= 8,
        "그림 슬롯이 문단 앞에 남아야 한다: {:?}",
        &p.char_offsets[..3]
    );
}

#[test]
fn incremental_typeset_matches_full_typeset() {
    let bytes = sample("HWP5-nopassword-123456.hwp");
    let mut inc = HwpDocument::from_bytes(&bytes).unwrap();
    inc.set_incremental_typeset(true);
    let mut full = HwpDocument::from_bytes(&bytes).unwrap();
    let long = "가나다라마바사아자차카타파하 ".repeat(6);
    let page_info = |d: &HwpDocument| -> Vec<String> {
        (0..d.page_count())
            .map(|p| {
                format!(
                    "{}\n{}",
                    d.get_page_info_native(p).unwrap(),
                    d.extract_page_text_native(p).unwrap()
                )
            })
            .collect()
    };
    for para in [300usize, 180, 60] {
        for d in [&mut inc, &mut full] {
            d.insert_text_native(0, para, 1, "가").unwrap();
            let snap = d.save_snapshot_native();
            d.insert_text_native(0, para, 2, &long).unwrap();
            d.restore_snapshot_native(snap).unwrap();
            d.insert_text_native(0, para, 2, &long).unwrap();
            d.delete_text_native(0, para, 2, long.chars().count())
                .unwrap();
        }
        assert_eq!(page_info(&inc), page_info(&full), "para {para}");
    }
}

/// 쪽마다 첫 네 줄(머리말 포함)과 쪽 정보.
fn page_heads(d: &HwpDocument) -> Vec<String> {
    (0..d.page_count())
        .map(|p| {
            let text = d.extract_page_text_native(p).unwrap();
            format!(
                "{}\n{}",
                d.get_page_info_native(p).unwrap(),
                text.lines().take(4).collect::<Vec<_>>().join("\n")
            )
        })
        .collect()
}

/// 뒤 구역만 고쳐 다시 조판할 때 앞 구역의 Even 머리말(쪽에 한 번도 안 쓰인 정의)을 잃었다.
/// dirty 가 아닌 구역은 쪽의 active_header 로, dirty 구역은 머리말 컨트롤로 carry 를 만들었기
/// 때문이다 — `exam_social.hwp` 2구역 한 글자에 2·4쪽 "2 (사회·문화)" → "2".
#[test]
fn editing_later_section_keeps_inherited_even_header() {
    let mut doc = HwpDocument::from_bytes(&sample("exam_social.hwp")).unwrap();
    let before = page_heads(&doc);
    doc.insert_text_native(1, 61, 1, "가").unwrap();
    doc.delete_text_native(1, 61, 1, 1).unwrap();
    assert_eq!(page_heads(&doc), before);
}

/// 스냅샷 복원(`rebuild_derived_state`)이 스타일을 변환본 보정 없이 다시 해소해,
/// HWP3 변환본은 되돌리기 한 번에 쪽 나눔이 바뀌었다.
#[test]
fn undo_on_hwp3_variant_restores_same_pages() {
    let mut doc = HwpDocument::from_bytes(&sample("hwp3-sample16-hwp5.hwp")).unwrap();
    let before = page_heads(&doc);
    let n = doc.document().sections[0].paragraphs.len();
    let p = (n / 2..n)
        .find(|&i| {
            doc.document().sections[0].paragraphs[i]
                .text
                .chars()
                .count()
                > 4
        })
        .unwrap();
    let snap = doc.save_snapshot_native();
    doc.insert_text_native(0, p, 1, "가").unwrap();
    doc.restore_snapshot_native(snap).unwrap();
    assert_eq!(page_heads(&doc), before);
}
