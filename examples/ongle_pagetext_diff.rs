//! 재직렬화 전후 쪽 텍스트 비교 (Ongle QA 2026-10-17 §7).
//!     cargo run --profile release-test --example ongle_pagetext_diff -- <file> [--hwpx] [--edit]
use rhwp::wasm_api::HwpDocument;

fn all_text(d: &HwpDocument) -> String {
    (0..d.page_count())
        .map(|p| d.extract_page_text_native(p).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\n")
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let bytes = std::fs::read(&args[0]).unwrap();
    let hwpx = args.iter().any(|a| a == "--hwpx");
    let mut a = HwpDocument::from_bytes(&bytes).unwrap();
    let before = all_text(&a);
    if args.iter().any(|a| a == "--edit") {
        // 1쪽 첫 본문 문단에 한 글자 넣었다 지운다 — 내용은 같고 raw_stream 만 버려진다.
        a.insert_text_native(0, 1, 0, "X").unwrap();
        a.delete_text_native(0, 1, 0, 1).unwrap();
    }
    if args.iter().any(|a| a == "--dump") {
        for (i, l) in all_text(&a).lines().enumerate().skip(170).take(20) {
            println!("mem {i}: {l:?}");
        }
    }
    let out = if hwpx {
        a.export_hwpx_native()
    } else {
        a.export_hwp_native()
    }
    .unwrap();
    let b = HwpDocument::from_bytes(&out).unwrap();
    let after = all_text(&b);
    let (la, lb): (Vec<&str>, Vec<&str>) = (before.lines().collect(), after.lines().collect());
    println!("lines {} -> {}", la.len(), lb.len());
    let mut shown = 0;
    for (i, (x, y)) in la.iter().zip(&lb).enumerate() {
        if x != y && shown < 12 {
            shown += 1;
            println!("line {i}\n  - {x:?}\n  + {y:?}");
        }
    }
}
