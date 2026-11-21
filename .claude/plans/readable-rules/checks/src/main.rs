//! 全合法手の往復検査: 局面ごとに「次の1手」を全て列挙し、
//! JKF → KIF/KI2/CSA → JKF で手順が変わらないか確かめる。
use shogi_core::{Move, Position};
use shogi_kifu_converter::converter::{ToCsa, ToKi2, ToKif};
use shogi_kifu_converter::error::ConvertError;
use shogi_kifu_converter::jkf::JsonKifuFormat;
use shogi_kifu_converter::parser::{parse_csa_str, parse_ki2_str, parse_kif_str};
use std::collections::BTreeMap;

const STARTS: [&str; 16] = ["平手","香落ち","右香落ち","角落ち","飛車落ち","飛香落ち","二枚落ち","三枚落ち","四枚落ち","五枚落ち","左五枚落ち","六枚落ち","左七枚落ち","右七枚落ち","八枚落ち","十枚落ち"];

struct Lcg(u64);
impl Lcg { fn next(&mut self, n: usize) -> usize { self.0 = self.0.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); ((self.0 >> 33) as usize) % n } }

fn main() {
    let games: usize = std::env::args().nth(1).map(|s| s.parse().unwrap()).unwrap_or(2);
    let depth: usize = std::env::args().nth(2).map(|s| s.parse().unwrap()).unwrap_or(40);
    let mut tally: BTreeMap<String, usize> = BTreeMap::new();
    let mut samples: BTreeMap<String, String> = BTreeMap::new();
    let mut rng = Lcg(1);
    for name in STARTS {
        let head = format!("手合割：{name}\n");
        let base = match parse_kif_str(&head) { Ok(j) => j, Err(e) => { *tally.entry(format!("start-unreadable {name}: {e}")).or_default() += 1; continue; } };
        let start = Position::try_from(&base).expect("start position");
        for _ in 0..games {
            let mut pos = start.clone();
            for _ in 0..depth {
                let legal = shogi_legality_lite::all_legal_moves_partial(pos.inner());
                if legal.is_empty() { break; }
                for &mv in &legal {
                    let mut p = pos.clone();
                    p.make_move(mv).unwrap();
                    check(name, &p, &mut tally, &mut samples);
                }
                let mv = legal[rng.next(legal.len())];
                pos.make_move(mv).unwrap();
            }
        }
    }
    for (k, v) in &tally { println!("{v:>8}  {k}"); }
    println!("--- samples");
    for (k, v) in &samples { println!("## {k}\n{v}"); }
}

fn moves_of(j: &JsonKifuFormat) -> Result<Vec<Move>, String> {
    Position::try_from(j).map(|p| p.moves().to_vec()).map_err(|e| format!("{e:?}"))
}

fn check(start: &str, p: &Position, tally: &mut BTreeMap<String, usize>, samples: &mut BTreeMap<String, String>) {
    let mut jkf = JsonKifuFormat::try_from(p).expect("jkf from position");
    // 手合割の名前を盤から畳む（consumer は normalize 済みの JKF を書く）
    if let Err(e) = jkf.normalize() { bump(tally, samples, &format!("normalize-fail {e}"), start, ""); return; }
    let want = p.moves().to_vec();
    let last_kind = match want.last() { Some(Move::Drop{..}) => "drop", Some(Move::Normal{promote:true,..}) => "promote", _ => "normal" };
    type W = fn(&JsonKifuFormat) -> Result<String, ConvertError>;
    type R = fn(&str) -> Result<JsonKifuFormat, shogi_kifu_converter::error::ParseError>;
    let fmts: [(&str, W, R); 3] = [
        ("kif", |j| j.try_to_kif_owned(), parse_kif_str),
        ("ki2", |j| j.try_to_ki2_owned(), parse_ki2_str),
        ("csa", |j| j.try_to_csa_owned(), parse_csa_str),
    ];
    for (f, w, r) in fmts {
        let key = match w(&jkf) {
            Err(ConvertError::UnspellableMove(_)) => format!("{f} write UnspellableMove"),
            Err(e) => format!("{f} write-error {:?}", std::mem::discriminant(&e)).replace("Discriminant", "") + &format!(" {e}").chars().take(60).collect::<String>(),
            Ok(text) => match r(&text) {
                Err(e) => { let k = format!("{f} reread-error"); bump(tally, samples, &k, start, &format!("{e}\n{text}")); continue; }
                Ok(back) => {
                    let side_ok = back.moves.iter().skip(1).zip(jkf.moves.iter().skip(1)).all(|(a,b)| a.move_.map(|m| m.color) == b.move_.map(|m| m.color));
                    match moves_of(&back) {
                        Ok(got) if got == want && side_ok => format!("{f} ok ({last_kind})"),
                        Ok(_) if !side_ok => { bump(tally, samples, &format!("{f} side-mismatch"), start, &text); continue; }
                        Ok(_) => { bump(tally, samples, &format!("{f} moves-differ"), start, &text); continue; }
                        Err(e) => { bump(tally, samples, &format!("{f} reread-unplayable"), start, &format!("{e}\n{text}")); continue; }
                    }
                }
            },
        };
        *tally.entry(key).or_default() += 1;
    }
}

fn bump(tally: &mut BTreeMap<String, usize>, samples: &mut BTreeMap<String, String>, key: &str, start: &str, body: &str) {
    *tally.entry(key.to_string()).or_default() += 1;
    samples.entry(key.to_string()).or_insert_with(|| format!("start={start}\n{}", body.lines().rev().take(6).collect::<Vec<_>>().into_iter().rev().collect::<Vec<_>>().join("\n")));
}
