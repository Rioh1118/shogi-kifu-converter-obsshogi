use criterion::{criterion_group, criterion_main, BatchSize, Criterion};
use shogi_kifu_converter::parser::parse_kif_str;
use std::fs;

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

fn bench_parse(c: &mut Criterion) {
    let cases: &[(&str, &str)] = &[
        ("bug_mega", "test/bug_mega.kif"),
        ("bug_big", "test/bug_big.kif"),
        ("Shogidokoro", "data/tests/kif/Shogidokoro.kif"),
    ];

    for (name, path) in cases {
        let src = load(path);
        let mut group = c.benchmark_group("parse_kif");
        if *name == "bug_mega" {
            group.sample_size(10);
        }
        group.bench_function(*name, |b| {
            b.iter_batched(
                || src.clone(),
                |s| parse_kif_str(&s).expect("parse failed"),
                BatchSize::SmallInput,
            );
        });
        group.finish();
    }
}

criterion_group!(benches, bench_parse);
criterion_main!(benches);
