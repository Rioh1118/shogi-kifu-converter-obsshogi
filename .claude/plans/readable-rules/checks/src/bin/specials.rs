//! 終局語の決定表: 終局の種類 16 × 書式 3 × 手番（先手・後手）× 手数（0手・1手）
//! の全セルで、書いて読み戻した終局の種類を表にする。
use shogi_kifu_converter::converter::{ToCsa, ToKi2, ToKif};
use shogi_kifu_converter::jkf::*;
use shogi_kifu_converter::parser::{parse_csa_str, parse_ki2_str, parse_kif_str};

const ALL: [&str; 16] = ["TORYO","CHUDAN","SENNICHITE","TIME_UP","ILLEGAL_MOVE","+ILLEGAL_ACTION","-ILLEGAL_ACTION","JISHOGI","KACHI","HIKIWAKE","MATTA","TSUMI","FUZUMI","ERROR","FUSENSHO","FUSENPAI"];

fn main() {
    // 平手（先手から）と 香落ち（後手=上手から）
    for (start, kif_head) in [("平手", "手合割：平手\n"), ("香落ち", "手合割：香落ち\n")] {
        for plies in [0usize, 1] {
            let mut body = kif_head.to_string();
            body.push_str("手数----指手---------消費時間--\n");
            if plies == 1 { body.push_str(if start == "平手" { "   1 ７六歩(77)\n" } else { "   1 ３四歩(33)\n" }); }
            let base = parse_kif_str(&body).expect("base");
            for sp in ALL {
                let special: MoveSpecial = serde_json::from_str(&format!("\"{sp}\"")).unwrap();
                let mut j = base.clone();
                j.moves.push(MoveFormat { special: Some(special), ..Default::default() });
                let mut row = format!("{start:<4} {plies}手 {sp:<16}");
                for f in ["kif", "ki2", "csa"] {
                    let text = match f { "kif" => j.try_to_kif_owned(), "ki2" => j.try_to_ki2_owned(), _ => j.try_to_csa_owned() };
                    let cell = match text {
                        Err(e) => format!("書けない({e})"),
                        Ok(t) => {
                            let back = match f { "kif" => parse_kif_str(&t), "ki2" => parse_ki2_str(&t), _ => parse_csa_str(&t) };
                            match back {
                                Err(e) => format!("読めない:{}", e.to_string().lines().next().unwrap_or("")),
                                Ok(b) => match b.moves.iter().rev().find_map(|m| m.special) {
                                    Some(s) => { let n = serde_json::to_string(&s).unwrap().trim_matches('"').to_string(); if n == sp { "=".into() } else { format!("→{n}") } }
                                    None => "→消える".into(),
                                },
                            }
                        }
                    };
                    row.push_str(&format!(" | {f}:{cell}"));
                }
                println!("{row}");
            }
        }
    }
}
