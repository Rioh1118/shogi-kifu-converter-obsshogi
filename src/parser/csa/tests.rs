use crate::jkf::*;
use crate::parser::parse_csa_str;

/// A broken position comes back as an error, not a panic: the consumer is a
/// Tauri command, and a panic there takes the application with it (R-REQ-004).
#[test]
fn broken_positions_are_errors() {
    for src in [
        // A king cannot be held in hand (R-CSA-006).
        "V2.2\nPI\nP+00OU\n+\n",
        // Positions are 1-9 in each direction (R-CSA-005).
        "V2.2\nP+05FU\n+\n",
        "V2.2\nP+50FU\n+\n",
        "V2.2\nPI00FU\n+\n",
        // More of a piece on the board than a set holds.
        "V2.2\nPI\nP+55FU\n+\n",
        // `AL` with more in the hands than a set holds.
        "V2.2\nPI\nP+00FU\nP-00AL\n+\n",
    ] {
        assert!(parse_csa_str(src).is_err(), "expected an error for {src:?}");
    }
}

fn read(src: &str) -> JsonKifuFormat {
    parse_csa_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"))
}

fn moves_and_specials(jkf: &JsonKifuFormat) -> Vec<String> {
    jkf.moves
        .iter()
        .skip(1)
        .map(|mf| match (mf.move_, mf.special) {
            (Some(m), _) => format!("{}{}", m.to.x, m.to.y),
            (None, Some(s)) => serde_json::to_string(&s).unwrap_or_default(),
            (None, None) => "-".to_owned(),
        })
        .collect()
}

/// GAP-040: a blank line anywhere, the first included, is no statement.
#[test]
fn a_blank_line_is_nothing() {
    let want = read("V2.2\nPI\n+\n+7776FU\n-3334FU\n");
    for src in [
        "\nV2.2\nPI\n+\n+7776FU\n-3334FU\n",
        "V2.2\n\nPI\n+\n+7776FU\n\n-3334FU\n\n",
    ] {
        assert_eq!(want, read(src), "{src:?}");
    }
}

/// GAP-041: V3.0 — the encoding line, `,T` on the move's line, `$TIME`, the
/// program's `'*` comments and `%MAX_MOVES` (R-CSA-007, R-CSA-009).
#[test]
fn a_v3_record_is_read() {
    let jkf = read(
        "'CSA encoding=UTF-8\nV3.0\nN+先手\n$TIME:900+0+5\nPI\n+\n+7776FU,T0\n-3334FU,T12.345\n'*読み筋 +2726FU\n%MAX_MOVES\n",
    );
    assert_eq!(vec!["76", "34", "\"MAX_MOVES\""], moves_and_specials(&jkf));
    assert_eq!(Some(&"900+0+5".to_owned()), jkf.header.get("TIME"));
    assert_eq!(12, jkf.moves[2].time.expect("a time").now.s);
    assert_eq!(
        Some(vec!["*読み筋 +2726FU".to_owned()]),
        jkf.moves[2].comments
    );
    assert_eq!(
        None, jkf.moves[0].comments,
        "the encoding line is no comment"
    );
}

/// GAP-044 / D30: a comment under the side-to-move line is the record's and
/// survives a round trip; one above it labels the file and is not kept.
#[test]
fn comments_below_the_side_to_move_line_are_kept() {
    let jkf =
        read("'対局者名\nV2.2\nPI\n+\n'開始局面のコメント\n+7776FU\n'** 30 -3334FU\n-3334FU\n");
    assert_eq!(
        Some(vec!["開始局面のコメント".to_owned()]),
        jkf.moves[0].comments
    );
    assert_eq!(
        Some(vec!["** 30 -3334FU".to_owned()]),
        jkf.moves[1].comments
    );
    assert_eq!(None, jkf.moves[2].comments);
    let back = read(&jkf.try_to_string(crate::Format::Csa).expect("writes CSA"));
    assert_eq!(jkf.moves, back.moves);
}

/// GAP-045: a comment after `,` runs to the end of the line, and what came
/// before it on the line stays (R-CSA-009).
#[test]
fn a_comment_after_a_comma_keeps_the_statement_before_it() {
    let jkf = read("V2.2\nPI\n+\n+7776FU\n%KACHI,'* -5000 win, +2726FU\n");
    assert_eq!(vec!["76", "\"KACHI\""], moves_and_specials(&jkf));
    assert_eq!(
        Some(vec!["* -5000 win, +2726FU".to_owned()]),
        jkf.moves[2].comments
    );
}

/// GAP-012: every V2.2 keyword reads as itself.
#[test]
fn every_special_keyword_is_read() {
    for word in [
        "TORYO",
        "CHUDAN",
        "SENNICHITE",
        "TIME_UP",
        "ILLEGAL_MOVE",
        "+ILLEGAL_ACTION",
        "-ILLEGAL_ACTION",
        "JISHOGI",
        "KACHI",
        "HIKIWAKE",
        "MATTA",
        "TSUMI",
        "FUZUMI",
        "ERROR",
    ] {
        let jkf = read(&format!("V2.2\nPI\n+\n+7776FU\n%{word}\n"));
        assert_eq!(
            vec!["76".to_owned(), format!("\"{word}\"")],
            moves_and_specials(&jkf),
            "{word}"
        );
    }
}

/// GAP-046: a header value is kept as written, and a keyword is a header line
/// wherever it sits — the 世界コンピュータ将棋選手権 relay writes `$END_TIME`
/// under the moves.
#[test]
fn header_values_are_kept_as_written() {
    let jkf = read("V2.2\n$START_TIME:2014/10/14 01:15:29\nPI\n+\n+7776FU\n%TORYO\n$END_TIME:2014/10/14 01:19:14\n");
    assert_eq!(
        Some(&"2014/10/14 01:15:29".to_owned()),
        jkf.header.get("開始日時")
    );
    assert_eq!(
        Some(&"2014/10/14 01:19:14".to_owned()),
        jkf.header.get("終了日時")
    );
}

/// D31: of several records separated by `/`, the first is the record.
#[test]
fn the_first_of_several_records_is_read() {
    let jkf = read("V2.2\nPI\n+\n+2726FU\n-3334FU\n%CHUDAN\n/\nV2.2\nPI\n+\n+7776FU\n");
    assert_eq!(vec!["26", "34", "\"CHUDAN\""], moves_and_specials(&jkf));
}

/// A statement this reader does not know is an error that names its line.
#[test]
fn an_unknown_statement_names_its_line() {
    let message = parse_csa_str("V2.2\nPI\n+\n+7776FU\nXYZZY\n-3334FU\n")
        .expect_err("XYZZY is no statement")
        .to_string();
    assert!(message.contains("line 5"), "{message}");
}

/// GAP-047: the times CSA has keywords for are written back — when the value
/// is in the shape V2.2 gives them, and not otherwise, since another program
/// refuses a `$START_TIME` it cannot read.
#[test]
fn the_times_csa_has_keywords_for_are_written() {
    let mut jkf = read("V2.2\nPI\n+\n+7776FU\n");
    for (key, value, kept) in [
        ("開始日時", "2014/10/14 01:15:29", true),
        ("終了日時", "2014/10/14", true),
        ("持ち時間", "00:25+00", true),
        ("開始日時", "2008/12/20(土) 10:00", false),
        ("持ち時間", "各6時間", false),
    ] {
        jkf.header.clear();
        jkf.header.insert(key.to_owned(), value.to_owned());
        let csa = jkf.try_to_string(crate::Format::Csa).expect("writes CSA");
        let back = read(&csa);
        assert_eq!(
            kept,
            back.header.get(key) == Some(&value.to_owned()),
            "{key}={value}: {csa}"
        );
    }
}

/// A rank of the bulk form is nine squares of three characters (R-CSA-006).
/// One cut short is missing pieces, not empty squares. The last empty square
/// may lose its trailing blank to the end of the line.
#[test]
fn a_rank_cut_short_is_an_error() {
    let upper = "P1-KY-KE-GI-KI-OU-KI-GI-KE-KY\nP2 * -HI *  *  *  *  * -KA * \nP3-FU-FU-FU-FU-FU-FU-FU-FU-FU\nP4 *  *  *  *  *  *  *  *  * \nP5 *  *  *  *  *  *  *  *  * \nP6 *  *  *  *  *  *  *  *  * \nP7+FU+FU+FU+FU+FU+FU+FU+FU+FU\nP8 * +KA *  *  *  *  * +HI *\n";
    let full = format!("V2.2\n{upper}P9+KY+KE+GI+KI+OU+KI+GI+KE+KY\n+\n");
    assert!(
        parse_csa_str(&full).is_ok(),
        "P8 without its last blank is whole"
    );
    let short = format!("V2.2\n{upper}P9+KY+KE+GI+KI+OU+KI+GI\n+\n");
    let message = parse_csa_str(&short)
        .expect_err("two squares are missing")
        .to_string();
    assert!(message.contains("line 10"), "{message}");
}

/// A comment with a comma in it comes back as one comment (R-CSA-008).
#[test]
fn a_comment_with_a_comma_comes_back_whole() {
    let mut jkf = read("V2.2\nPI\n+\n+7776FU\n");
    jkf.moves[1].comments = Some(vec!["7六歩, 角道を開ける".to_owned()]);
    let back = read(&jkf.try_to_string(crate::Format::Csa).expect("writes CSA"));
    assert_eq!(jkf.moves[1].comments, back.moves[1].comments);
}
