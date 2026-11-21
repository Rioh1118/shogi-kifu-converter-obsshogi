//! 小さな入力を書式ごとに読んで、通るかを1行で出す。
use shogi_kifu_converter::{parser::parse_str, Format};
fn main() {
    let cases: &[(Format, &str, &str)] = &[
        (Format::Ki2, "全角数字 ▲△", "▲２六歩 △３四歩\n"),
        (Format::Ki2, "半角数字", "▲26歩 △34歩\n"),
        (Format::Ki2, "▽ の後手", "▲２六歩 ▽３四歩\n"),
        (Format::Ki2, "まで2手で不詰", "▲２六歩 △３四歩\nまで2手で不詰\n"),
        (Format::Ki2, "まで2手で後手の勝ち(投了)", "▲２六歩 △３四歩\nまで2手で後手の勝ち\n"),
        (Format::Ki2, "まで2手で先手の勝ち(名前と手番が逆)", "▲２六歩 △３四歩\nまで2手で先手の勝ち\n"),
        (Format::Ki2, "まで2手で詰", "▲２六歩 △３四歩\nまで2手で詰\n"),
        (Format::Ki2, "まで2手詰", "▲２六歩 △３四歩\nまで2手詰\n"),
        (Format::Ki2, "まで2手で封じ手", "▲２六歩 △３四歩\nまで2手で封じ手\n"),
        (Format::Ki2, "まで2手で後手の反則負け", "▲２六歩 △３四歩\nまで2手で後手の反則負け\n"),
        (Format::Ki2, "まで2手で千日手", "▲２六歩 △３四歩\nまで2手で千日手\n"),
        (Format::Ki2, "まで2手で持将棋", "▲２六歩 △３四歩\nまで2手で持将棋\n"),
        (Format::Kif, "同銀　(79)", "1 ７六歩(77)\n2 ３四歩(33)\n3 ２二角成(88)\n4 同銀　(31)\n"),
        (Format::Kif, "同　銀(31)", "1 ７六歩(77)\n2 ３四歩(33)\n3 ２二角成(88)\n4 同　銀(31)\n"),
        (Format::Kif, "同銀(31)", "1 ７六歩(77)\n2 ３四歩(33)\n3 ２二角成(88)\n4 同銀(31)\n"),
        (Format::Kif, "封じ手", "1 ７六歩(77)\n2 封じ手\n"),
        (Format::Kif, "半角数字の移動先", "1 76歩(77)\n"),
        (Format::Kif, "王", "手合割：その他\n後手の持駒：なし\n  ９ ８ ７ ６ ５ ４ ３ ２ １\n+---------------------------+\n| ・ ・ ・ ・v王 ・ ・ ・ ・|一\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n| ・ ・ ・ ・ 王 ・ ・ ・ ・|九\n+---------------------------+\n先手の持駒：なし\n"),
        (Format::Kif, "\\r\\r\\n", "手合割：平手\r\r\n先手：a\r\r\n手数----指手---------消費時間--\r\r\n   1 ７六歩(77)\r\r\n"),
        (Format::Kif, "竜", "手合割：平手\n1 ７六歩(77)\n2 ３四歩(33)\n3 ２二角成(88)\n"),
        (Format::Csa, "先頭の空行", "\nV2.2\nPI\n+\n+7776FU\n"),
        (Format::Csa, "末尾の空行", "V2.2\nPI\n+\n+7776FU\n\n"),
        (Format::Csa, "途中の空行", "V2.2\nPI\n+\n+7776FU\n\n-3334FU\n"),
        (Format::Csa, "V3.0 の文字コード行", "'CSA encoding=UTF-8\nV3.0\nPI\n+\n+7776FU\n"),
        (Format::Csa, "V3.0 の ,T", "V3.0\nPI\n+\n+7776FU,T0\n"),
        (Format::Csa, "V3.0 の $TIME", "V3.0\n$TIME:900+0+5\nPI\n+\n+7776FU\n"),
        (Format::Csa, "V3.0 の '*", "V3.0\nPI\n+\n+7776FU\n'*コメント\n"),
        (Format::Csa, "%MAX_MOVES", "V3.0\nPI\n+\n+7776FU\n%MAX_MOVES\n"),
        (Format::Csa, "CRLF", "V2.2\r\nPI\r\n+\r\n+7776FU\r\n"),
    ];
    for (format, label, text) in cases {
        let r = std::panic::catch_unwind(|| parse_str(*format, text));
        let v = match r {
            Err(_) => "PANIC".to_string(),
            Ok(Err(e)) => format!("ERR  {}", e.to_string().lines().next().unwrap_or("").chars().take(80).collect::<String>()),
            Ok(Ok(j)) => format!("OK   moves={} special={:?}", j.moves.iter().filter(|m| m.move_.is_some()).count(), j.moves.iter().rev().find_map(|m| m.special)),
        };
        println!("{format:?}\t{label}\t{v}");
    }
}
