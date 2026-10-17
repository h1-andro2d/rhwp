//! 파일을 열어 HWP/HWPX 로 내보낸다 (raw_stream 버림 옵션).
//!     cargo run --profile release-test --example ongle_export -- <in> <out.hwp|out.hwpx> [--drop-raw]
use rhwp::wasm_api::HwpDocument;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut d = HwpDocument::from_bytes(&std::fs::read(&a[0]).unwrap()).unwrap();
    if a.iter().any(|x| x == "--drop-raw") {
        for s in &mut d.document_mut().sections {
            s.raw_stream = None;
        }
    }
    let out = if a[1].ends_with(".hwpx") {
        d.export_hwpx_native()
    } else {
        d.export_hwp_native()
    };
    std::fs::write(&a[1], out.unwrap()).unwrap();
}
