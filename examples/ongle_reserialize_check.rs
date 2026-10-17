//! 구역 재직렬화 왕복 검사 (Ongle QA 2026-10-17 §7 "편집한 구역의 컨트롤 위치 이동").
//!
//!     cargo run --profile release-test --example ongle_reserialize_check -- <file|dir> [--hwpx] [-v]
//!
//! 모든 구역의 raw_stream 을 버려 "편집된 구역"과 같은 경로로 HWP(기본) 또는 HWPX 로
//! 다시 쓰고, 다시 읽어 문단마다 (텍스트, 컨트롤 종류와 텍스트 안 위치)를 원본과 비교한다.
//! 컨트롤 위치는 `char_offsets` 갭으로 복원한 "몇 번째 글자 앞" 값이다.

use rhwp::model::paragraph::Paragraph;
use rhwp::wasm_api::HwpDocument;
use std::path::{Path, PathBuf};

/// 컨트롤 i 가 놓인 텍스트 위치(그 앞에 있는 글자 수)를 char_offsets 갭으로 복원한다.
/// 각 컨트롤 = 8 code unit. 필드 끝(0x04) 표시도 8 을 차지하므로 갭/8 개를 그대로 센다.
fn control_slots(p: &Paragraph) -> Vec<usize> {
    let mut slots = Vec::new();
    let mut prev_end = 0u32;
    for (i, ch) in p.text.chars().enumerate() {
        let off = p.char_offsets.get(i).copied().unwrap_or(prev_end);
        let mut gap = off.saturating_sub(prev_end) / 8;
        while gap > 0 {
            slots.push(i);
            gap -= 1;
        }
        prev_end = off + if ch == '\t' { 8 } else { ch.len_utf16() as u32 };
    }
    slots.push(usize::MAX); // 끝 표시 — 나머지 컨트롤은 문단 끝
    slots
}

fn describe(p: &Paragraph) -> String {
    let kinds: Vec<String> = p
        .controls
        .iter()
        .map(|c| {
            format!("{:?}", c)
                .chars()
                .take_while(|c| *c != '(')
                .collect()
        })
        .collect();
    let fields: Vec<String> = p
        .field_ranges
        .iter()
        .map(|f| {
            format!(
                "c{}:{}..{}",
                f.control_idx, f.start_char_idx, f.end_char_idx
            )
        })
        .collect();
    let slots = control_slots(p);
    let deep = if std::env::var("DEEP").is_ok() {
        format!(
            " shape={} segs={:?} cs={:?}",
            p.para_shape_id,
            p.line_segs
                .iter()
                .map(|l| (l.text_start, l.vertical_pos, l.line_height, l.segment_width))
                .collect::<Vec<_>>(),
            p.char_shapes
                .iter()
                .map(|c| (c.start_pos, c.char_shape_id))
                .collect::<Vec<_>>()
        )
    } else {
        String::new()
    };
    format!(
        "text={:?} ctrls={:?} gaps={:?} fields={:?}{}",
        p.text.chars().take(60).collect::<String>(),
        kinds,
        &slots[..slots.len() - 1],
        fields,
        deep
    )
}

fn check(path: &Path, hwpx: bool, verbose: bool) -> (usize, usize) {
    let bytes = std::fs::read(path).unwrap();
    let Ok(mut a) = HwpDocument::from_bytes(&bytes) else {
        return (0, 0);
    };
    if a.document().sections.iter().any(|s| s.raw_stream.is_none()) && !hwpx {
        // HWPX 원본 등은 애초에 재직렬화 경로라 이 검사가 따로 할 일이 없다.
    }
    for s in &mut a.document_mut().sections {
        s.raw_stream = None;
    }
    let out = if hwpx {
        a.export_hwpx_native()
    } else {
        a.export_hwp_native()
    };
    let Ok(out) = out else {
        println!("{}: export failed", path.display());
        return (0, 1);
    };
    let b = HwpDocument::from_bytes(&out).expect("reparse");
    let (da, db) = (a.document(), b.document());
    let mut total = 0;
    let mut bad = 0;
    for (si, (sa, sb)) in da.sections.iter().zip(&db.sections).enumerate() {
        if sa.paragraphs.len() != sb.paragraphs.len() {
            println!(
                "{}: s{} paragraph count {} -> {}",
                path.display(),
                si,
                sa.paragraphs.len(),
                sb.paragraphs.len()
            );
            bad += 1;
        }
        for (pi, (pa, pb)) in sa.paragraphs.iter().zip(&sb.paragraphs).enumerate() {
            total += 1;
            let (x, y) = (describe(pa), describe(pb));
            if x != y {
                bad += 1;
                if verbose || bad <= 3 {
                    println!(
                        "{}: s{}:p{}\n  before {}\n  after  {}",
                        path.display(),
                        si,
                        pi,
                        x,
                        y
                    );
                    if verbose {
                        println!(
                            "  offsets before {:?}\n  offsets after  {:?}",
                            pa.char_offsets, pb.char_offsets
                        );
                    }
                }
            }
        }
    }
    (total, bad)
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let hwpx = args.iter().any(|a| a == "--hwpx");
    let verbose = args.iter().any(|a| a == "-v");
    let root = PathBuf::from(args.iter().find(|a| !a.starts_with('-')).unwrap());
    let files: Vec<PathBuf> = if root.is_dir() {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "hwp"))
            .filter(|p| {
                std::fs::metadata(p)
                    .map(|m| m.len() < 8 << 20)
                    .unwrap_or(false)
            })
            .collect();
        v.sort();
        v
    } else {
        vec![root]
    };
    let (mut files_bad, mut paras_bad) = (0, 0);
    for f in &files {
        let r = std::panic::catch_unwind(|| check(f, hwpx, verbose));
        match r {
            Ok((_, bad)) if bad > 0 => {
                files_bad += 1;
                paras_bad += bad;
                println!("{}: {} paragraphs differ", f.display(), bad);
            }
            Ok(_) => {}
            Err(_) => println!("{}: panic", f.display()),
        }
    }
    println!(
        "files={} files_with_diffs={} paragraphs_with_diffs={}",
        files.len(),
        files_bad,
        paras_bad
    );
}
