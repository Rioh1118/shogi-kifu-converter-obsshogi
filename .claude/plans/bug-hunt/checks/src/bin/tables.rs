//! 決定表: 手合割・ヘッダ・消費時間・コメントの置き場 × 書式 を、書いて読み戻して埋める。
use shogi_kifu_converter::jkf::*;
use shogi_kifu_converter::{parser::parse_str, Format};
use std::collections::HashMap;

const WRITE: [Format; 4] = [Format::Kif, Format::Ki2, Format::Csa, Format::Jkf];

fn roundtrip(j: &JsonKifuFormat, f: Format) -> Result<JsonKifuFormat, String> {
    let r = std::panic::catch_unwind(|| {
        let t = j.try_to_string(f).map_err(|e| format!("書けない({e})"))?;
        parse_str(f, &t).map_err(|e| format!("読めない({})", e.to_string().lines().next().unwrap_or("").chars().take(50).collect::<String>()))
    });
    r.unwrap_or_else(|_| Err("PANIC".into()))
}

fn preset_name(p: Preset) -> String { serde_json::to_string(&p).unwrap().trim_matches('"').to_string() }

fn main() {
    let which = std::env::args().nth(1).unwrap_or_default();
    let base = parse_str(Format::Kif, "手合割：平手\n").unwrap();
    if which == "preset" {
        println!("手合割\t経路\t{}", WRITE.map(|f| format!("{f:?}")).join("\t"));
        let presets: Vec<Preset> = ["HIRATE","KY","KY_R","KA","HI","HIKY","2","3","4","5","5_L","6","7_L","7_R","8","10","OTHER"].iter().map(|s| serde_json::from_str(&format!("\"{s}\"")).unwrap()).collect();
        for p in presets {
            // 名前で持つ JKF
            let mut j = base.clone();
            j.initial = Some(Initial { preset: p, data: None });
            let row: Vec<String> = WRITE.iter().map(|&f| match roundtrip(&j, f) {
                Ok(b) => { let q = b.initial.map(|i| i.preset).unwrap_or(Preset::PresetHirate); if q == p { "=".into() } else { format!("→{}", preset_name(q)) } }
                Err(e) => e,
            }).collect();
            println!("{}\t名前\t{}", preset_name(p), row.join("\t"));
            // 盤面で持つ JKF（その他 + data）
            if p != Preset::PresetOther {
                let pos = shogi_core::Position::try_from(&j).unwrap();
                let init: Initial = pos.initial_position().into();
                let mut k = base.clone();
                k.initial = Some(Initial { preset: Preset::PresetOther, data: init.data });
                let row: Vec<String> = WRITE.iter().map(|&f| match roundtrip(&k, f) {
                    Ok(b) => { let q = b.initial.map(|i| i.preset).unwrap_or(Preset::PresetHirate); if q == p { "名前に畳む".into() } else { format!("→{}", preset_name(q)) } }
                    Err(e) => e,
                }).collect();
                println!("{}\t盤面\t{}", preset_name(p), row.join("\t"));
            }
        }
    }
    if which == "header" {
        let keys = ["開始日時","終了日時","棋戦","戦型","表題","持ち時間","秒読み","消費時間","場所","掲載","備考","先手","後手","下手","上手","先手省略名","後手省略名","記録係","作者","tags"];
        println!("キー\t{}", WRITE.map(|f| format!("{f:?}")).join("\t"));
        for k in keys {
            for (label, v) in [("普通の値", "値 abc 2024/01/02"), ("改行入り", "1行目\n2行目"), ("コロン入り", "a：b:c"), ("空", "")] {
                let mut j = base.clone();
                j.header = HashMap::from([(k.to_string(), v.to_string())]);
                let row: Vec<String> = WRITE.iter().map(|&f| match roundtrip(&j, f) {
                    Ok(b) => { let mut got: Vec<_> = b.header.iter().filter(|(_, x)| !x.is_empty() || v.is_empty()).map(|(a, x)| format!("{a}={x:?}")).collect(); got.sort();
                        match b.header.get(k) { Some(x) if x == v => "=".into(), Some(x) => format!("値→{x:?}"), None if got.is_empty() => "消える".into(), None => format!("→{}", got.join(",")) } }
                    Err(e) => e,
                }).collect();
                if label == "普通の値" || row.iter().any(|c| c != "=") { println!("{k}/{label}\t{}", row.join("\t")); }
            }
        }
    }
    if which == "time" {
        println!("消費時間\t{}", WRITE.map(|f| format!("{f:?}")).join("\t"));
        for (label, h, m, s) in [("0:01", None, 0u16, 1u16), ("59:59", None, 59, 59), ("90:00", None, 90, 0), ("1時間0分0秒(h付き)", Some(1u16), 0, 0), ("256:00", None, 256, 0)] {
            let mut j = parse_str(Format::Kif, "手合割：平手\n   1 ７六歩(77)\n   2 ３四歩(33)\n").unwrap();
            j.moves[1].time = Some(Time { now: TimeFormat { h, m, s }, total: TimeFormat::default() });
            let _ = j.normalize();
            let want = j.moves[1].time.map(|t| t.now);
            let row: Vec<String> = WRITE.iter().map(|&f| match roundtrip(&j, f) {
                Ok(b) => match (b.moves.get(1).and_then(|m| m.time).map(|t| t.now), want) {
                    (Some(g), Some(w)) if g == w => "=".into(),
                    (Some(g), _) => format!("→{:?}:{}:{}", g.h, g.m, g.s),
                    (None, _) => "消える".into(),
                },
                Err(e) => e,
            }).collect();
            println!("{label}\t{}", row.join("\t"));
        }
    }
    if which == "comment" {
        println!("コメント\t{}", WRITE.map(|f| format!("{f:?}")).join("\t"));
        let src = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ３四歩(33)+\n   3 投了\n\n変化：2手\n   2 ８四歩(83)\n";
        type Place = (&'static str, fn(&mut JsonKifuFormat) -> &mut MoveFormat);
        let places: [Place; 5] = [
            ("初期局面", |j| &mut j.moves[0]),
            ("1手目の後", |j| &mut j.moves[1]),
            ("終局の後", |j| &mut j.moves[3]),
            ("変化の1手目", |j| &mut j.moves[2].forks.as_mut().unwrap()[0][0]),
            ("変化の前の手(分岐元)", |j| &mut j.moves[2]),
        ];
        for (label, at) in places {
            for (kind, text) in [("1行", "メモ"), ("2行", "1行目\n2行目"), ("&で始まる", "&しおり"), ("空行入り", "a\n\nb"), ("*で始まる", "*星")] {
                let mut j = parse_str(Format::Kif, src).unwrap();
                at(&mut j).comments = Some(text.split('\n').map(String::from).collect());
                let want = at(&mut j).comments.clone();
                let row: Vec<String> = WRITE.iter().map(|&f| match roundtrip(&j, f) {
                    Ok(mut b) => {
                        let got = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| at(&mut b).comments.clone())).unwrap_or(Some(vec!["<枠が無い>".into()]));
                        if got == want { "=".into() } else { format!("→{:?}", got.map(|v| v.join("⏎"))) }
                    }
                    Err(e) => e,
                }).collect();
                if kind == "1行" || row.iter().any(|c| c != "=") { println!("{label}/{kind}\t{}", row.join("\t")); }
            }
        }
    }
}
