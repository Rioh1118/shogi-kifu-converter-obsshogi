//! ファイルを読み、各書式に書いて読み戻し、本譜と変化の手が同じかを見る。
use shogi_kifu_converter::{jkf::*, parser::{parse_file, parse_str}, Format};
fn key(ms: &[MoveFormat], out: &mut Vec<String>) {
    for m in ms {
        out.push(format!("{:?}|{:?}", m.move_.map(|x| (x.color as u8, x.from.map(|f| (f.x, f.y)), (x.to.x, x.to.y), x.piece, x.promote.unwrap_or(false))), m.special));
        for f in m.forks.iter().flatten() { out.push("fork{".into()); key(f, out); out.push("}".into()); }
    }
}
fn main() {
    for path in std::env::args().skip(1) {
        let j = match parse_file(&path) { Ok(j) => j, Err(e) => { println!("READERR {path} {e}"); continue; } };
        let mut want = vec![]; key(&j.moves, &mut want);
        let mut row = vec![];
        for f in [Format::Kif, Format::Ki2, Format::Csa, Format::Jkf] {
            let cell = match j.try_to_string(f) {
                Err(e) => format!("書けない({e})"),
                Ok(t) => match parse_str(f, &t) {
                    Err(e) => format!("読めない({})", e.to_string().lines().next().unwrap_or("")),
                    Ok(b) => { let mut got = vec![]; key(&b.moves, &mut got);
                        if got == want { "=".into() } else if f == Format::Csa && j.moves.iter().any(|m| m.forks.is_some()) { "=(変化以外)?".into() } else {
                            let n = want.iter().zip(&got).position(|(a, b)| a != b).unwrap_or(want.len().min(got.len()));
                            format!("違う@{n}: {:?} / {:?}", want.get(n), got.get(n)) } }
                },
            };
            row.push(format!("{f:?}:{}", cell.chars().take(150).collect::<String>()));
        }
        println!("{}\t{}", std::path::Path::new(&path).file_name().unwrap().to_string_lossy(), row.join(" | "));
    }
}
