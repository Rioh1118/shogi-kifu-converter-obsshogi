use shogi_kifu_converter::{parser::parse_str, Format};
fn main() {
    for (label, kif, want) in [
        ("二枚落ち（USI 仕様の例）", "手合割：二枚落ち\n   1 ６二玉(51)\n   2 ７六歩(77)\n   3 ３二銀(31)\n", "sfen lnsgkgsnl/9/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1 moves 5a6b 7g7f 3a3b"),
        ("平手", "手合割：平手\n   1 ７六歩(77)\n   2 ３四歩(33)\n   3 ２二角成(88)\n   4 同　銀(31)\n   5 ４五角打\n", "startpos moves 7g7f 3c3d 8h2b+ 3a2b B*4e"),
        ("香落ち（0手）", "手合割：香落ち\n", "sfen lnsgkgsn1/1r5b1/ppppppppp/9/9/9/PPPPPPPPP/1B5R1/LNSGKGSNL w - 1"),
    ] {
        let j = parse_str(Format::Kif, kif).unwrap();
        let got = j.try_to_usi_owned().unwrap();
        println!("{} {label}\n  want {want}\n  got  {got}", if got == want { "OK  " } else { "DIFF" });
    }
}
