//! 駒落ちの分岐: 変化の手の手番が本譜と同じ規則で決まるか、書いて読み戻して変わらないか。
use shogi_kifu_converter::converter::{ToCsa, ToKi2, ToKif};
use shogi_kifu_converter::parser::{parse_ki2_str, parse_kif_str};
fn main() {
    for head in ["手合割：平手", "手合割：香落ち"] {
        let (a, b, c) = if head.ends_with("平手") { ("７六歩(77)", "３四歩(33)", "８四歩(83)") } else { ("３四歩(33)", "７六歩(77)", "２六歩(27)") };
        let kif = format!("{head}\n手数----指手---------消費時間--\n   1 {a}\n   2 {b}+\n   3 投了\n\n変化：2手\n   2 {c}\n   3 投了\n");
        let j = match parse_kif_str(&kif) { Ok(j) => j, Err(e) => { println!("{head}: read error {e}"); continue; } };
        let main_colors: Vec<_> = j.moves.iter().filter_map(|m| m.move_.map(|x| x.color as u8)).collect();
        let fork_colors: Vec<_> = j.moves[2].forks.iter().flatten().flatten().filter_map(|m| m.move_.map(|x| x.color as u8)).collect();
        println!("{head}: main colors {main_colors:?} fork colors {fork_colors:?}");
        let k2 = j.try_to_kif_owned().unwrap();
        let back = parse_kif_str(&k2).unwrap();
        println!("  kif roundtrip same: {}", serde_json::to_string(&back).unwrap() == serde_json::to_string(&j).unwrap());
        match j.try_to_ki2_owned() { Ok(t) => { let b = parse_ki2_str(&t); println!("  ki2 text:\n{}\n  ki2 reread: {:?}", t, b.map(|b| serde_json::to_string(&b.moves).unwrap())); } Err(e) => println!("  ki2 write err {e}") }
        let _ = j.try_to_csa_owned();
    }
}
