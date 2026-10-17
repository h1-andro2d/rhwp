//! 문단 하나의 IR 을 찍는다.  cargo run --profile release-test --example ongle_dump_para -- <file> <sec> <para>
use rhwp::wasm_api::HwpDocument;
fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let d = HwpDocument::from_bytes(&std::fs::read(&a[0]).unwrap()).unwrap();
    let p = &d.document().sections[a[1].parse::<usize>().unwrap()].paragraphs
        [a[2].parse::<usize>().unwrap()];
    println!(
        "text={:?}\noffsets={:?}\nchar_shapes={:?}\nfields={:?}\nchar_count={}",
        p.text, p.char_offsets, p.char_shapes, p.field_ranges, p.char_count
    );
    for c in &p.controls {
        let s = format!("{:?}", c);
        println!("ctrl {}", &s[..s.len().min(160)]);
    }
}
