//! 되돌리기가 편집 전 쪽을 그대로 되살리는지 본다.
//!     cargo run --profile release-test --example ongle_undo_check -- <file>
//! 문단 몇 곳에서 (스냅샷 → 글자 넣기 → 복원) 후 전체 쪽 텍스트·쪽 정보를 편집 전과 비교한다.
use rhwp::wasm_api::HwpDocument;

fn pages(d: &HwpDocument) -> Vec<String> {
    (0..d.page_count())
        .map(|p| {
            format!(
                "{}\n{}",
                d.get_page_info_native(p).unwrap_or_default(),
                d.extract_page_text_native(p).unwrap_or_default()
            )
        })
        .collect()
}

fn main() {
    let a: Vec<String> = std::env::args().skip(1).collect();
    let mut d = HwpDocument::from_bytes(&std::fs::read(&a[0]).unwrap()).unwrap();
    if std::env::var("INC").is_ok() {
        d.set_incremental_typeset(true);
    }
    let t0 = pages(&d);
    let n = d.document().sections[0].paragraphs.len();
    let mut bad = 0;
    // 인자로 "sec:para" 를 주면 그곳만, 아니면 구역 0 의 고른 네 곳.
    let spots: Vec<(usize, usize)> = if a.len() > 1 {
        a[1..]
            .iter()
            .map(|x| {
                let (s, p) = x.split_once(':').unwrap();
                (s.parse().unwrap(), p.parse().unwrap())
            })
            .collect()
    } else {
        (1..=4).map(|k| (0, (n * k) / 5)).collect()
    };
    let text = std::env::var("TEXT")
        .unwrap_or_else(|_| "가나다라마바사아자차카타파하 가나다라마바사아자차카타파하".into());
    for (s, p) in spots {
        if d.document().sections[s].paragraphs[p].text.chars().count() < 2 {
            continue;
        }
        let snap = d.save_snapshot_native();
        d.insert_text_native(s, p, 2, &text).unwrap();
        d.restore_snapshot_native(snap).unwrap();
        let t = pages(&d);
        let first = t0.iter().zip(&t).position(|(x, y)| x != y);
        if t != t0 {
            bad += 1;
            if let Some(i) = first {
                let (a, b) = (&t0[i], &t[i]);
                let (ac, bc): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
                let at = ac.iter().zip(&bc).position(|(x, y)| x != y).unwrap_or(0);
                let win = |v: &[char]| {
                    v[at.saturating_sub(150)..(at + 250).min(v.len())]
                        .iter()
                        .collect::<String>()
                };
                println!("  pre ..{}..\n  now ..{}..", win(&ac), win(&bc));
            }
        }
        println!(
            "para {p}: pages {} -> {} first_diff_page={first:?}",
            t0.len(),
            t.len()
        );
    }
    println!("undo_mismatches={bad}");
}
