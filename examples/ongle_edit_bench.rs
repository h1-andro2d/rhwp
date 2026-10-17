//! 편집 한 글자 비용 측정 (Ongle QA 2026-10-17 §7).
//!
//!     cargo run --profile release-test --example ongle_edit_bench -- <file> [keys]
//!
//! 문단이 가장 많은 구역의 가운데쯤 본문 문단에서 한 글자 넣기·지우기를 keys 번씩,
//! 그리고 "한 글자 넣고 되돌리기(스냅샷 복원)" 를 keys 번 재서 p50/max 를 찍는다.
//! RHWP_INCREMENTAL_TYPESET=1 이면 증분 조판(적재 때부터 기록). RHWP_2424_PROFILE=1 은 단계별 시간.

use rhwp::wasm_api::HwpDocument;
use std::time::Instant;

fn ms(t: Instant) -> f64 {
    t.elapsed().as_secs_f64() * 1000.0
}

fn stats(v: &mut [f64]) -> (f64, f64) {
    v.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (v[v.len() / 2], v[v.len() - 1])
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let path = &args[1];
    let keys: usize = args.get(2).and_then(|s| s.parse().ok()).unwrap_or(10);
    let bytes = std::fs::read(path).expect("read");
    let t = Instant::now();
    let mut doc = HwpDocument::from_bytes(&bytes).expect("parse");
    let open_ms = ms(t);
    let (sec, paras) = doc
        .document()
        .sections
        .iter()
        .enumerate()
        .map(|(i, s)| (i, s.paragraphs.len()))
        .max_by_key(|&(_, n)| n)
        .unwrap();
    let para = {
        let d = doc.document();
        (paras / 2..paras)
            .find(|&i| {
                let p = &d.sections[sec].paragraphs[i];
                p.controls.is_empty() && p.text.chars().count() > 10
            })
            .unwrap_or(0)
    };
    let mut ins = Vec::new();
    let mut del = Vec::new();
    let mut undo = Vec::new();
    for k in 0..keys {
        let t = Instant::now();
        doc.insert_text_native(sec, para, k, "가").expect("insert");
        ins.push(ms(t));
    }
    for k in (0..keys).rev() {
        let t = Instant::now();
        doc.delete_text_native(sec, para, k, 1).expect("delete");
        del.push(ms(t));
    }
    for _ in 0..keys {
        let snap = doc.save_snapshot_native();
        doc.insert_text_native(sec, para, 1, "가").expect("insert");
        let t = Instant::now();
        doc.restore_snapshot_native(snap).expect("restore");
        undo.push(ms(t));
        doc.discard_snapshot_native(snap);
    }
    let (i50, imax) = stats(&mut ins);
    let (d50, dmax) = stats(&mut del);
    let (u50, umax) = stats(&mut undo);
    println!(
        "{}\tpages={}\tsections={}\ttarget=s{}:p{}/{}\topen={:.0}\tinsert p50={:.2} max={:.2}\tdelete p50={:.2} max={:.2}\tundo p50={:.2} max={:.2}",
        path.rsplit('/').next().unwrap(),
        doc.page_count(),
        doc.document().sections.len(),
        sec,
        para,
        paras,
        open_ms,
        i50,
        imax,
        d50,
        dmax,
        u50,
        umax
    );
}
