//! 連盟の「棋譜の表記方法」の例（Kifu-for-JS の normalizer-relative.test.ts）で、
//! KI2 の書き手が書く相対位置の語と、読み戻した移動元を確かめる。
use shogi_kifu_converter::{jkf::*, parser::parse_str, Format};
fn case(piece: &str, coords: &[u32], to: u32, expected: &[&str]) {
    for (&from, &want) in coords.iter().zip(expected) {
        let mut board = vec![vec![serde_json::json!({}); 9]; 9];
        for &c in coords { board[(c / 10 - 1) as usize][(c % 10 - 1) as usize] = serde_json::json!({"color": 0, "kind": piece}); }
        let j = serde_json::json!({"header": {}, "initial": {"preset": "OTHER", "data": {"color": 0, "board": board, "hands": [{"FU":0,"KY":0,"KE":0,"GI":0,"KI":0,"KA":0,"HI":0}, {"FU":0,"KY":0,"KE":0,"GI":0,"KI":0,"KA":0,"HI":0}]}},
            "moves": [{}, {"move": {"color": 0, "from": {"x": from / 10, "y": from % 10}, "to": {"x": to / 10, "y": to % 10}, "piece": piece}}]});
        let mut jkf: JsonKifuFormat = serde_json::from_value(j).unwrap();
        let label = format!("{piece} {coords:?}→{to} from {from}");
        if let Err(e) = jkf.normalize() { println!("NORMALIZE-ERR {label}: {e}"); continue; }
        let text = match jkf.try_to_string(Format::Ki2) { Ok(t) => t, Err(e) => { println!("WRITE-ERR {label} want {want}: {e}"); continue; } };
        let line = text.lines().find(|l| l.starts_with('▲')).unwrap_or("").to_string();
        let back = parse_str(Format::Ki2, &text);
        let got_from = back.as_ref().ok().and_then(|b| b.moves.get(1)).and_then(|m| m.move_).and_then(|m| m.from).map(|f| f.x as u32 * 10 + f.y as u32);
        let ok_word = line.ends_with(want) || (want.is_empty());
        let status = if ok_word && got_from == Some(from) { "OK  " } else { "DIFF" };
        println!("{status} {label}: want 「{want}」 wrote {line} reread_from={got_from:?}{}", back.err().map(|e| format!(" ERR {e}")).unwrap_or_default());
    }
}
fn main() {
    case("KI", &[93, 72], 82, &["上", "寄"]); case("KI", &[43, 31], 32, &["上", "引"]); case("KI", &[56, 45], 55, &["上", "寄"]);
    case("GI", &[89, 77], 88, &["上", "引"]); case("GI", &[49, 27], 38, &["上", "引"]);
    case("KI", &[92, 72], 81, &["左", "右"]); case("KI", &[32, 12], 22, &["左", "右"]); case("GI", &[65, 45], 56, &["左", "右"]);
    case("KI", &[89, 79], 78, &["左", "直"]); case("GI", &[39, 29], 38, &["直", "右"]);
    case("KI", &[63, 53, 43], 52, &["左", "直", "右"]); case("TO", &[79, 89, 99, 98, 87], 88, &["右", "直", "左上", "寄", "引"]);
    case("GI", &[29, 17, 39, 37], 28, &["直", "右", "左上", "左引"]);
    case("RY", &[91, 84], 82, &["引", "上"]); case("RY", &[23, 52], 43, &["寄", "引"]); case("RY", &[55, 15], 35, &["左", "右"]);
    case("RY", &[99, 89], 88, &["左", "右"]); case("RY", &[28, 19], 17, &["左", "右"]);
    case("UM", &[91, 81], 82, &["左", "右"]); case("UM", &[95, 63], 85, &["寄", "引"]); case("UM", &[11, 34], 12, &["引", "上"]);
    case("UM", &[99, 59], 77, &["左", "右"]); case("UM", &[47, 18], 29, &["左", "右"]); case("UM", &[93, 74], 92, &["左", "右"]);
}
