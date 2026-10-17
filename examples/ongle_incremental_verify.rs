//! 증분 조판 결과가 처음부터 조판한 결과와 같은지 실문서로 확인한다.
//!
//!     RHWP_INCREMENTAL_VERIFY=1 cargo run --profile release-test --example ongle_incremental_verify -- <file|dir> [targets]
//!
//! 문서마다 구역 전체에 고르게 퍼진 문단 몇 개를 골라 한 글자 넣기·여러 줄 넣기·지우기·
//! 문단 나누기 뒤 이어 쓰기를 한다. 편집마다 엔진이 증분 결과와 처음부터 조판한 결과를
//! 비교해 stderr 에 RHWP_INCREMENTAL_VERIFY_OK / _FAIL 을 찍는다. 끝에 쪽 수와 전체 쪽 텍스트를
//! 증분을 끈 같은 편집 문서와도 비교한다(구역 간 쪽 번호 보정 등 조판 뒤 단계까지).

use rhwp::wasm_api::HwpDocument;
use std::path::{Path, PathBuf};

const LONG: &str = "가나다라마바사아자차카타파하 가나다라마바사아자차카타파하 가나다라마바사아자차카타파하 가나다라마바사아자차카타파하";

fn targets(doc: &HwpDocument, n: usize) -> Vec<(usize, usize)> {
    let all: Vec<(usize, usize)> = doc
        .document()
        .sections
        .iter()
        .enumerate()
        .flat_map(|(si, s)| {
            s.paragraphs
                .iter()
                .enumerate()
                .filter(|(_, p)| p.text.chars().count() >= 4)
                .map(move |(pi, _)| (si, pi))
        })
        .collect();
    if all.is_empty() {
        return Vec::new();
    }
    // 고르게 퍼진 n 개, 뒤에서부터 — 문단 나누기가 앞 대상의 번호를 밀지 않게.
    let mut v: Vec<_> = (0..n)
        .map(|k| all[((2 * k + 1) * all.len()) / (2 * n)])
        .collect();
    v.dedup();
    v.reverse();
    v
}

fn edit(doc: &mut HwpDocument, (s, p): (usize, usize)) -> Result<(), String> {
    let e = |r: Result<String, rhwp::error::HwpError>| r.map(|_| ()).map_err(|e| format!("{e:?}"));
    // 한 글자 넣고 되돌리기(스냅샷 복원) → 다시 넣기
    let snap = doc.save_snapshot_native();
    e(doc.insert_text_native(s, p, 2, "가"))?;
    e(doc.restore_snapshot_native(snap))?;
    doc.discard_snapshot_native(snap);
    e(doc.insert_text_native(s, p, 2, "가"))?;
    // 여러 줄 넣고 되돌리기 → 다시 넣었다 지우기
    let snap = doc.save_snapshot_native();
    e(doc.insert_text_native(s, p, 3, LONG))?;
    e(doc.restore_snapshot_native(snap))?;
    doc.discard_snapshot_native(snap);
    e(doc.insert_text_native(s, p, 3, LONG))?;
    e(doc.delete_text_native(s, p, 3, LONG.chars().count()))?;
    e(doc.delete_text_native(s, p, 2, 1))?;
    // 문단 나누기(문단 수가 바뀌어 처음부터 조판) 뒤 다음 문단 편집은 새 기록으로 이어진다.
    e(doc.split_paragraph_native(s, p, 2, None))?;
    e(doc.insert_text_native(s, p + 1, 0, "나"))?;
    Ok(())
}

thread_local! {
    static SNAP: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
}

/// `edit` 를 한 단계씩 (STEPCHECK 용).
fn edit_step(doc: &mut HwpDocument, (s, p): (usize, usize), step: usize) -> Result<(), String> {
    let e = |r: Result<String, rhwp::error::HwpError>| r.map(|_| ()).map_err(|e| format!("{e:?}"));
    match step {
        0 | 4 => {
            SNAP.with(|c| c.set(doc.save_snapshot_native()));
            Ok(())
        }
        1 | 3 => e(doc.insert_text_native(s, p, 2, "가")),
        2 | 6 => e(doc.restore_snapshot_native(SNAP.with(|c| c.get()))),
        5 | 7 => e(doc.insert_text_native(s, p, 3, LONG)),
        8 => e(doc.delete_text_native(s, p, 3, LONG.chars().count())),
        9 => e(doc.delete_text_native(s, p, 2, 1)),
        10 => e(doc.split_paragraph_native(s, p, 2, None)),
        11 => e(doc.insert_text_native(s, p + 1, 0, "나")),
        _ => Ok(()),
    }
}

fn all_text(d: &HwpDocument) -> String {
    (0..d.page_count())
        .map(|p| d.extract_page_text_native(p).unwrap_or_default())
        .collect::<Vec<_>>()
        .join("\u{c}")
}

fn run(path: &Path, n: usize) -> String {
    let bytes = std::fs::read(path).unwrap();
    let Ok(mut inc) = HwpDocument::from_bytes(&bytes) else {
        return "parse_error".into();
    };
    let mut full = HwpDocument::from_bytes(&bytes).unwrap();
    inc.set_incremental_typeset(true);
    let mut done = 0;
    for t in targets(&inc, n) {
        if t.1 >= inc.document().sections[t.0].paragraphs.len() {
            continue;
        }
        if std::env::var("STEPCHECK").is_ok() {
            // 같은 편집을 한 단계씩 번갈아 하며 처음 어긋나는 단계를 찾는다.
            let mut before_insert = String::new();
            for step in 0..12 {
                if step == 1 {
                    before_insert = all_text(&full);
                }
                let ra = edit_step(&mut inc, t, step);
                let rb = edit_step(&mut full, t, step);
                if ra.is_err() || rb.is_err() {
                    break;
                }
                let (ti, tf) = (all_text(&inc), all_text(&full));
                if ti != tf {
                    return format!(
                        "first_mismatch target={t:?} step={step} incremental_matches_pre_edit={} rebuild_matches_pre_edit={}",
                        ti == before_insert,
                        tf == before_insert
                    );
                }
            }
            done += 1;
            continue;
        }
        if let Err(e) = edit(&mut inc, t) {
            return format!("edit_error {t:?} {e}");
        }
        edit(&mut full, t).unwrap();
        done += 1;
    }
    let same_pages = inc.page_count() == full.page_count();
    let same_text = all_text(&inc) == all_text(&full);
    format!(
        "edits={} pages={} same_pages={} same_text={}",
        done,
        inc.page_count(),
        same_pages,
        same_text
    )
}

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let root = PathBuf::from(&args[0]);
    let n: usize = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(6);
    let files: Vec<PathBuf> = if root.is_dir() {
        let mut v: Vec<PathBuf> = std::fs::read_dir(&root)
            .unwrap()
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().is_some_and(|e| e == "hwp" || e == "hwpx"))
            .filter(|p| {
                std::fs::metadata(p)
                    .map(|m| m.len() < 6 << 20)
                    .unwrap_or(false)
            })
            .collect();
        v.sort();
        v
    } else {
        vec![root]
    };
    for f in &files {
        eprintln!("FILE {}", f.display());
        let r = std::panic::catch_unwind(|| run(f, n)).unwrap_or_else(|_| "panic".into());
        println!("{}\t{}", f.display(), r);
        eprintln!("RESULT {}", r);
    }
}
