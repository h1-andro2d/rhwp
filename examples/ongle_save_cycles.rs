//! 편집→저장→다시 열기를 되풀이할 때 HWP3 변환본 보정값이 누적되는지 본다.
//!     cargo run --profile release-test --example ongle_save_cycles -- <file> [cycles] [--hwpx]
use rhwp::wasm_api::HwpDocument;

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let cycles: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(3);
    let hwpx = args.iter().any(|a| a == "--hwpx");
    let mut bytes = std::fs::read(&args[0]).unwrap();
    for c in 0..=cycles {
        let mut d = HwpDocument::from_bytes(&bytes).unwrap();
        let doc = d.document();
        let s0 = &doc.sections[0];
        let vpos: Vec<i32> = [70usize, 71, 200]
            .iter()
            .filter_map(|&i| {
                s0.paragraphs
                    .get(i)
                    .and_then(|p| p.line_segs.first())
                    .map(|l| l.vertical_pos)
            })
            .collect();
        let tail: Vec<(usize, i32, i32)> = doc
            .doc_info
            .para_shapes
            .iter()
            .enumerate()
            .skip(51)
            .map(|(i, p)| (i, p.spacing_before, p.spacing_after))
            .collect();
        if !tail.is_empty() {
            println!("  added shapes after reload {:?}", tail);
        }
        let sb: Vec<(usize, i32, i32)> = doc
            .doc_info
            .para_shapes
            .iter()
            .enumerate()
            .filter(|(_, p)| p.spacing_before != 0 || p.spacing_after != 0)
            .take(4)
            .map(|(i, p)| (i, p.spacing_before, p.spacing_after))
            .collect();
        println!(
            "cycle {c}: variant={} pages={} margin_bottom={} vpos(p70,p71,p200)={:?} para_shape(id,sb,sa)={:?} docinfo_raw={}",
            doc.is_hwp3_variant,
            d.page_count(),
            s0.section_def.page_def.margin_bottom,
            vpos,
            sb,
            doc.doc_info.raw_stream.is_some(),
        );
        if args.iter().any(|a| a == "--bold") {
            // 새 글자 모양이 생겨 DocInfo 가 raw 통과 대신 다시 쓰이게 한다.
            d.apply_char_format_native(0, 1, 0, 1, r#"{"bold":true}"#)
                .unwrap();
        }
        if args.iter().any(|a| a == "--para") {
            // 문단 모양을 바꿔 새 ParaShape 를 만들게 한다 (spacing 은 그대로 복제된다).
            let pi = d.document().sections[0]
                .paragraphs
                .iter()
                .position(|p| p.para_shape_id == 4)
                .unwrap();
            let before = d.document().sections[0].paragraphs[pi].para_shape_id;
            d.apply_para_format_native(0, pi, r#"{"alignment":"center"}"#)
                .unwrap();
            let id = d.document().sections[0].paragraphs[pi].para_shape_id as usize;
            let ps = &d.document().doc_info.para_shapes[id];
            println!(
                "  para {pi}: shape {before} -> {id} sb={} sa={}",
                ps.spacing_before, ps.spacing_after
            );
        }
        d.insert_text_native(0, 1, 0, "X").unwrap();
        d.delete_text_native(0, 1, 0, 1).unwrap();
        bytes = if hwpx {
            d.export_hwpx_native()
        } else {
            d.export_hwp_native()
        }
        .unwrap();
    }
}
