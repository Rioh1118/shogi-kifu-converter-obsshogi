//! Read and write benchmarks.
//!
//! The write side is here because the consumer's save path goes through this
//! crate's converters (R-REQ-002) and its cost is not proportional to the read
//! side: spelling KI2 has to work out a disambiguating suffix for every move
//! (R-NOT-004), which is board work, while reading KIF just parses text.
//!
//! Only fixtures under `data/tests/` are required. `test/` is `.gitignore`d, so
//! a benchmark that needs it does not run on a clean checkout.
//!
//! **Measure natively.** On a machine whose `rustc` host triple does not match
//! the CPU, `cargo bench` builds for the host and runs under emulation; the
//! numbers come out roughly twice as large and the ranking between passes can
//! change. Pass `--target` explicitly if `rustc -vV` disagrees with `uname -m`.

use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use shogi_kifu_converter::converter::{ToCsa, ToKi2, ToKif};
use shogi_kifu_converter::jkf::JsonKifuFormat;
use shogi_kifu_converter::parser::parse_kif_str;
use std::fs;
use std::path::Path;

fn load(path: &str) -> String {
    let bytes = fs::read(path).unwrap_or_else(|e| panic!("failed to read {path}: {e}"));
    let (cow, _, had_errors) = encoding_rs::SHIFT_JIS.decode(&bytes);
    if had_errors {
        let (cow_utf8, _, _) = encoding_rs::UTF_8.decode(&bytes);
        cow_utf8.into_owned()
    } else {
        cow.into_owned()
    }
}

/// The tracked fixtures, plus the two large ones when they happen to be there.
///
/// `bug_mega.kif` is the one that shows the write side's cost — 16k moves and
/// 1347 branches — but it lives in the ignored `test/`, so it is optional.
fn cases() -> Vec<(String, String)> {
    let mut out = vec![(
        "Shogidokoro".to_owned(),
        "data/tests/kif/Shogidokoro.kif".to_owned(),
    )];
    for name in ["bug_mega", "bug_big"] {
        let path = format!("test/{name}.kif");
        if Path::new(&path).exists() {
            out.push((name.to_owned(), path));
        }
    }
    out
}

fn bench_parse(c: &mut Criterion) {
    for (name, path) in cases() {
        let src = load(&path);
        let mut group = c.benchmark_group("parse_kif");
        if name == "bug_mega" {
            group.sample_size(10);
        }
        group.bench_function(&name, |b| {
            b.iter_batched(
                || src.clone(),
                |s| parse_kif_str(&s).expect("parse failed"),
                BatchSize::SmallInput,
            );
        });
        group.finish();
    }
}

/// A writer under test: the group name and something that runs it.
type Writer = (&'static str, fn(&JsonKifuFormat) -> bool);

fn bench_write(c: &mut Criterion) {
    for (name, path) in cases() {
        let jkf: JsonKifuFormat = parse_kif_str(&load(&path)).expect("parse failed");
        let writers: [Writer; 3] = [
            ("to_ki2", |j| j.try_to_ki2_owned().is_ok()),
            ("to_kif", |j| j.try_to_kif_owned().is_ok()),
            ("to_csa", |j| j.try_to_csa_owned().is_ok()),
        ];
        for (format, run) in writers {
            let mut group = c.benchmark_group(format);
            if name == "bug_mega" {
                group.sample_size(10);
            }
            group.bench_function(&name, |b| b.iter(|| run(&jkf)));
            group.finish();
        }
    }
}

criterion_group!(benches, bench_parse, bench_write);
criterion_main!(benches);
