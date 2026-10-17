//! 뒤 구역만 고쳤을 때 앞 구역에서 물려받는 머리말이 유지되는지.
//!     cargo run --profile release-test --example ongle_header_carry -- <file> <sec> <para>
use rhwp::wasm_api::HwpDocument;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut d = HwpDocument::from_bytes(&std::fs::read(&a[0]).unwrap()).unwrap();
    let (s, p): (usize, usize) = (a[1].parse().unwrap(), a[2].parse().unwrap());
    let head = |d: &HwpDocument| -> Vec<String> {
        (0..d.page_count())
            .map(|i| {
                d.extract_page_text_native(i)
                    .unwrap()
                    .lines()
                    .take(4)
                    .collect::<Vec<_>>()
                    .join(" | ")
            })
            .collect()
    };
    let before = head(&d);
    d.insert_text_native(s, p, 1, "가").unwrap();
    let after = head(&d);
    for (i, (x, y)) in before.iter().zip(&after).enumerate() {
        if x != y {
            println!("page {i}\n  before: {x}\n  after:  {y}");
        }
    }
}
