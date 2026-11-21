use super::*;
use crate::handicap::HIRATE_BOARD;

// Which line the message names when a board diagram breaks. `opt(board)`
// unwinds the whole diagram on a recoverable error, so the position
// `convert_error` prints is the start of the diagram — a line that is
// intact, for a file whose fault is nine lines below it. D1 asks for the
// line that broke.
#[test]
fn a_broken_board_names_the_line_that_broke() {
    const HEAD: &str = concat!(
        "後手の持駒：なし\n",
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
    );
    const ROWS: [&str; 9] = [
        "| ・ ・ ・ ・v玉 ・ ・ ・ ・|一",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八",
        "| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九",
    ];
    let build = |rows: [&str; 9], close: &str| {
        format!("{HEAD}{}\n{close}\n先手の持駒：なし\n", rows.join("\n"))
    };
    const FRAME: &str = "+---------------------------+";
    assert!(
        crate::parser::parse_kif_str(&build(ROWS, FRAME)).is_ok(),
        "the unbroken diagram must read"
    );

    // A rank heading that is not one (`九` → `1`), eight lines in.
    let mut broken = ROWS;
    broken[8] = "| ・ ・ ・ ・ 玉 ・ ・ ・ ・|1";
    let message = match crate::parser::parse_kif_str(&build(broken, FRAME)) {
        Err(crate::error::ParseError::Kif(message)) => message,
        other => panic!("a broken rank must not read: {other:?}"),
    };
    assert!(
        message.contains("this rank of the board cannot be read"),
        "{message}"
    );
    // Line 12 of the file, not line 2 where the diagram opens.
    assert!(message.contains("at line 12"), "{message}");

    // The same for the frame the diagram never closes with.
    let message = match crate::parser::parse_kif_str(&build(ROWS, "+-----+")) {
        Err(crate::error::ParseError::Kif(message)) => message,
        other => panic!("an unclosed board must not read: {other:?}"),
    };
    assert!(
        message.contains("the board has no frame under its last rank"),
        "{message}"
    );
    assert!(message.contains("at line 13"), "{message}");

    // A diagram whose first two lines disagree: either one says a board
    // starts here, so the other one missing is a fault in the board and not
    // prose that happens to look like one.
    let head_broken = |record: &str, what: &str| match crate::parser::parse_kif_str(record) {
        Err(crate::error::ParseError::Kif(message)) => {
            assert!(message.contains(what), "{message}");
        }
        other => panic!("{what}: {other:?}"),
    };
    head_broken(
        &format!(
            "後手の持駒：なし\n  ９ ８ ７ ６ ５ ４ ３ ２\n{FRAME}\n{}\n{FRAME}\n",
            ROWS.join("\n")
        ),
        "the board's file numbers cannot be read",
    );
    head_broken(
        &format!(
            "後手の持駒：なし\n  ９ ８ ７ ６ ５ ４ ３ ２ １\n{}\n{FRAME}\n",
            ROWS.join("\n")
        ),
        "the board has no frame under its files",
    );
    // A `+` line that opens no diagram is still prose.
    assert!(crate::parser::parse_kif_str(
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n+7776FU\n"
    )
    .is_ok());

    // A file that stops inside the diagram has no line to name, so it says
    // that instead of pointing at the line after the last one.
    let message = match crate::parser::parse_kif_str(&format!("{HEAD}{}\n", ROWS[..4].join("\n"))) {
        Err(crate::error::ParseError::Kif(message)) => message,
        other => panic!("a diagram cut short must not read: {other:?}"),
    };
    assert!(
        message.contains("the file ends inside the board"),
        "{message}"
    );
}

#[test]
fn parse_move_time_format() {
    assert!(move_time_format("").is_err());
    assert_eq!(
        Ok((
            "",
            TimeFormat {
                h: None,
                m: 0,
                s: 16
            }
        )),
        move_time_format("0:16")
    );
    assert_eq!(
        Ok((
            "",
            TimeFormat {
                h: Some(0),
                m: 0,
                s: 16
            }
        )),
        move_time_format("00:00:16")
    );
}

// `KIF_SPECIAL_WORDS` is a hand-written list, so nothing makes it follow
// `MoveSpecial`. A word the writer produces and the reader does not look for
// is a record this crate writes and cannot read back — and the consumer
// writes with `to_kif` on every save (R-REQ-002).
#[test]
fn the_reader_looks_for_every_word_the_writer_writes() {
    for special in MoveSpecial::ALL {
        let Some(word) = special.kif_word() else {
            continue;
        };
        assert!(
            KIF_SPECIAL_WORDS.contains(&word),
            "{special:?} writes {word:?}, which the reader does not look for"
        );
    }
    // And nothing in the list is a word no special move names.
    for word in KIF_SPECIAL_WORDS {
        assert!(
            MoveSpecial::from_kif_word(word, Color::Black).is_some(),
            "{word:?} reads as no special move"
        );
    }
}

// Every diagnostic this reader can print, with an input that prints it.
//
// A message no input reaches is a claim about the reader that nobody
// checked: `this move's origin square cannot be read` spent two rounds
// unreachable because its caller replaced it, and `this move's clock cannot
// be read` could not fire at all. Both read as if the reader told the user
// what was wrong, and it did not.
#[test]
fn every_diagnostic_has_an_input_that_prints_it() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
    const BOARD: [&str; 12] = [
        "  ９ ８ ７ ６ ５ ４ ３ ２ １",
        "+---------------------------+",
        "| ・ ・ ・ ・ ・ ・ ・ ・v玉|一",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八",
        "| ・ ・ ・ ・ ・ ・ ・ ・ 玉|九",
        "+---------------------------+",
    ];
    let board_with = |broken: usize, line: &str| {
        BOARD
            .iter()
            .enumerate()
            .map(|(i, l)| format!("{}\n", if i == broken { line } else { l }))
            .collect::<String>()
    };
    let printed = [
        (
            "a 変化 block with no moves under it",
            format!("{HEAD}   2 ８四歩(83)\n\n変化：2手\n"),
        ),
        (
            "the board has no file numbers",
            board_with(0, "後手番").replacen("後手番\n", "", 1),
        ),
        (
            "the board has no frame under its files",
            board_with(1, "後手番"),
        ),
        (
            "the board has no frame under its last rank",
            board_with(11, "後手番"),
        ),
        (
            "the file ends inside the board",
            BOARD[..6].join("\n").to_string() + "\n",
        ),
        (
            "this hand line cannot be read",
            String::from("先手の持駒：歩零\n"),
        ),
        (
            "this line runs into the one below it",
            format!("{HEAD}   2 ８四歩(83)   3 ７五歩(76)\n"),
        ),
        ("this move cannot be read", format!("{HEAD}   2 パス\n")),
        (
            "this move has no origin square",
            format!("{HEAD}   2 ８四歩\n"),
        ),
        (
            "this move's origin square cannot be read",
            format!("{HEAD}   2 ８四歩(00)\n"),
        ),
        (
            "this rank of the board cannot be read",
            board_with(5, "| ・ ・|五"),
        ),
        (
            "this record states one hand twice",
            format!("先手の持駒：飛\n先手の持駒：角\n{}", board_with(99, "")),
        ),
        (
            "this record states a hand but has no board for it",
            String::from("先手の持駒：飛\n"),
        ),
        (
            "this line's ply number is too large to read",
            format!("{HEAD}99999999999999999999 ８四歩(83)\n"),
        ),
    ];
    for (message, record) in printed {
        let printed = match parse_kif_str(&record) {
            Err(crate::error::ParseError::Kif(printed)) => printed,
            other => panic!("{message}: {other:?}\n{record}"),
        };
        assert!(printed.contains(message), "{message}\n{printed}");
    }
    // And the reader that shares them prints them too.
    assert!(parse_ki2_str("先手の持駒：飛\n")
        .expect_err("a hand with no board")
        .to_string()
        .contains("no board for it"));
}

// A blank line or a note does not end the header block. A reader that stops
// at the first line none of its own rules take makes the *previous* line
// decide who owns the next one: everything below goes to the move list's
// skip, which takes it whole. The record then comes back `Ok` with the
// handicap, the hands and the side to move gone — and the writer saves that
// over the original (D1, D4, GAP-007).
#[test]
fn a_blank_line_or_a_note_does_not_end_the_header_block() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    for gap in ["", "\n", "第1譜\n", "\n\n第1譜\n\n"] {
        let kif = format!(
            "先手：山田太郎\n{gap}後手：田中一郎\n棋戦：竜王戦\n手合割：香落ち\n\
                 手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        );
        let read = parse_kif_str(&kif).unwrap_or_else(|e| panic!("{gap:?}: {e}"));
        assert_eq!(3, read.header.len(), "{gap:?}: {:?}", read.header);
        // The handicap decides who moves first (R-HC-001 / R-RULE-006), so
        // losing it reverses every side in the game.
        assert_eq!(
            Some(Preset::PresetKY),
            read.initial.map(|initial| initial.preset),
            "{gap:?}"
        );
        let ki2 = format!("先手：山田太郎\n{gap}後手：田中一郎\n手合割：香落ち\n△３四歩\n");
        assert_eq!(
            2,
            parse_ki2_str(&ki2)
                .unwrap_or_else(|e| panic!("{gap:?} as a KI2: {e}"))
                .header
                .len(),
            "{gap:?} as a KI2"
        );
    }
    // Including between the board and the lines that belong to it: the hands
    // and the side to move are part of the diagram's block (R-KIF-014 / D6).
    const BOARD: &str = concat!(
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・v玉|一\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ 玉|九\n",
        "+---------------------------+\n",
    );
    for gap in ["", "\n", "図：第1図\n"] {
        let record = format!("後手の持駒：なし\n{BOARD}{gap}先手の持駒：飛\n{gap}後手番\n");
        let data = parse_kif_str(&record)
            .unwrap_or_else(|e| panic!("{gap:?}: {e}"))
            .initial
            .expect("a position")
            .data
            .expect("a board");
        assert_eq!(1, data.hands[0].HI, "{gap:?}: the hand went");
        assert_eq!(Color::White, data.color, "{gap:?}: the side to move went");
    }
}

// A hand belongs to a board. Where the diagram could not be read the pieces
// go with it — `initial.data` is `None` and they are simply gone — and the
// record comes back as an empty 平手 that the writer then saves over the
// original (D4, GAP-007).
//
// Only where something is lost. An empty hand states nothing the preset does
// not already say, and a reader that refuses it drops records tsshogi opens
// (R-KIF-014).
#[test]
fn a_hand_with_no_board_is_reported_rather_than_dropped() {
    for record in [
        "後手の持駒：飛二 角\n先手の持駒：",
        "後手の持駒：飛二 角\n先手の持駒：\n",
        "後手の持駒：飛二 角\n先手の持駒：金",
        "後手の持駒：飛二 角\n先手の持駒：金\n",
    ] {
        let message = crate::parser::parse_kif_str(record)
            .expect_err(record)
            .to_string();
        assert!(message.contains("no board for it"), "{message}");
        // And the message names the hand line, not the head of the record.
        assert!(message.contains("後手の持駒：飛二 角"), "{message}");
        assert!(crate::parser::parse_ki2_str(record).is_err(), "{record:?}");
    }
    // The line that made the refusal necessary is the one named, even where
    // an empty hand came first.
    let message = crate::parser::parse_kif_str("先手の持駒：なし\n後手の持駒：飛\n")
        .expect_err("a hand with no board is refused")
        .to_string();
    assert!(message.contains("後手の持駒：飛"), "{message}");
    assert!(!message.contains("先手の持駒：なし"), "{message}");
    // Nothing to lose, so nothing to refuse.
    for record in [
            "手合割：平手\n後手の持駒：なし\n手数----指手---------消費時間--\n   1 ７六歩(77)\n",
            "手合割：香落ち\n上手の持駒：なし\n下手の持駒：なし\n手数----指手---------消費時間--\n   1 ３四歩(33)\n",
            "後手の持駒：\n手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n",
        ] {
            assert!(crate::parser::parse_kif_str(record).is_ok(), "{record:?}");
        }
}

// A hand line whose count cannot be read must not fall through to the
// key-value rule: that files the whole line under `header` and leaves the
// hand empty — including the pieces written before the broken one. A drop
// from that hand then fails to normalize, and the message names the move
// rather than the line that actually broke.
#[test]
fn a_hand_line_that_cannot_be_read_is_an_error_not_an_empty_hand() {
    const BOARD: &str = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ ・ ・ ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
";
    for hand in ["歩十九 角二", "角二 歩十九", "歩零", "歩十八 と三"] {
        let kif = format!("{BOARD}先手の持駒：{hand}\n手数----指手---------消費時間--\n");
        let err = crate::parser::parse_kif_str(&kif)
            .err()
            .unwrap_or_else(|| panic!("{hand:?} was accepted"));
        assert!(
            matches!(err, crate::error::ParseError::Kif(_)),
            "{hand:?} gave {err:?}"
        );
    }
}

// The counts come from the file, so they can add up past what a `u8` holds.
// Wrapping would turn a broken hand into a small plausible one and hand it
// to the rest of the crate as a fact.
#[test]
fn a_hand_that_overflows_a_u8_does_not_wrap() {
    // 18 pawns fifteen times: 270, which is 14 once it wraps.
    let overflowing = "歩十八 ".repeat(15);
    assert!(
        information_value_hand(overflowing.trim_end()).is_err(),
        "a hand of 270 must not parse"
    );
}

// A record states each hand once. Stated twice, the two answers a reader can
// give — keep the last, or add them up — both change the position without
// saying so: pieces disappear or pieces appear that nobody wrote. D1 puts
// that below being refused, and the corpus has no record that states one
// twice (0 of 609).
//
// An empty hand is the exception, since it says nothing the other line does
// not: `先手の持駒：なし` above a diagram and `先手の持駒：飛` below it is one
// hand stated once.
#[test]
fn a_hand_stated_twice_is_reported_rather_than_merged() {
    use crate::parser::parse_kif_str;
    const BOARD: &str = concat!(
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・v玉|一\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ 玉|九\n",
        "+---------------------------+\n",
    );
    // Twice with pieces, in one block and across the diagram.
    for record in [
        format!("先手の持駒：飛\n先手の持駒：角\n{BOARD}"),
        format!("先手の持駒：飛\n{BOARD}先手の持駒：角\n"),
    ] {
        let message = parse_kif_str(&record).expect_err(&record).to_string();
        assert!(message.contains("states one hand twice"), "{message}");
    }
    // Once, with an empty line for the other side of the diagram.
    let data = parse_kif_str(&format!("先手の持駒：なし\n{BOARD}先手の持駒：飛\n"))
        .expect("reads")
        .initial
        .expect("a position")
        .data
        .expect("a board");
    assert_eq!(1, data.hands[0].HI);
}

// A line the move list has no shape for is skipped whole — one line, not
// two. The parser is anchored on the line's first character, and a newline
// must not be allowed to be it: starting on the newline of a *blank* line
// takes the line after it as this line's content, so a blank line in the
// middle of a record destroys the move that follows it (R-KIF-002).
//
// `skippable_line` tries `blank_line` first, which hides this. The guard is
// what keeps the two orderings from meaning different things.
#[test]
fn a_skipped_line_never_swallows_the_line_after_it() {
    assert_eq!(
        Ok(("   2 ３四歩(33)\n", "変化：2")),
        not_move_line(WhereABoardCouldBe(false), "変化：2\n   2 ３四歩(33)\n")
    );
    assert!(not_move_line(WhereABoardCouldBe(false), "\n   2 ３四歩(33)\n").is_err());
    assert!(not_move_line(WhereABoardCouldBe(false), "\r\n   2 ３四歩(33)\n").is_err());
    // Padding in front of a blank line does not make it a line with
    // something on it.
    assert!(not_move_line(WhereABoardCouldBe(false), "　\t \n   2 ３四歩(33)\n").is_err());
}

// What a line is does not change with how far in it starts. KIF writes its
// move lines three columns in (R-KIF-008), and a note or a `まで…` lined up
// with them is the same line it would be at column 0 — but a reader that
// asks only at column 0 hands it to the prose skip, and what it said goes
// with it. The KI2 side of this is
// `ki2::tests::a_line_is_the_line_it_is_however_far_in_it_starts`.
#[test]
fn kif_reads_a_line_however_far_in_it_starts() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KIF: &str =
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
    const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
    for pad in ["", " ", "   ", "\t", "　", "\u{a0}"] {
        // A comment and a bookmark are kept (R-KIF-010 / R-KIF-011), and
        // prose after them is skipped, whichever column they start in.
        let kif = parse_kif_str(&format!("{KIF}{pad}*コメント\n{pad}&しおり\n{pad}感想戦\n"))
            .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
        assert_eq!(
            Some(&vec![String::from("コメント"), String::from("&しおり")]),
            kif.moves[2].comments.as_ref(),
            "{pad:?}: {:?}",
            kif.moves
        );
        // And the same content reads the same way as `.ki2` (D18).
        let ki2 = parse_ki2_str(&format!("{KI2}{pad}*コメント\n{pad}&しおり\n{pad}感想戦\n"))
            .unwrap_or_else(|e| panic!("{pad:?} as .ki2: {e}"));
        assert_eq!(kif.moves[2].comments, ki2.moves[2].comments, "{pad:?}");
        // A move line is a move line however it is indented.
        let indented = parse_kif_str(&format!(
                "手合割：平手\n手数----指手---------消費時間--\n{pad}   1 ７六歩(77)\n{pad}   2 ８四歩(83)\n"
            ))
            .unwrap_or_else(|e| panic!("{pad:?} on a move line: {e}"));
        assert_eq!(2, indented.moves.len() - 1, "{pad:?}: the moves went");
        // So is a fork header, and the diagnostic under it survives.
        assert!(
            parse_kif_str(&format!("{KIF}\n{pad}変化：2手\n")).is_err(),
            "{pad:?}: `a 変化 block with no moves under it` went quiet"
        );
    }
}

#[test]
fn parse_comment_line() {
    assert!(comment_line("").is_err());
    assert!(comment_line("not comment\n").is_err());
    assert_eq!(
        Ok(("", String::from(" comment"))),
        comment_line("# comment\n")
    );
    // A file need not end with a newline. Insisting on one dropped the last
    // line and said nothing, so the record came back short.
    assert_eq!(
        Ok(("", String::from(" comment"))),
        comment_line("# comment")
    );
}

#[test]
fn parse_not_move_line() {
    assert!(not_move_line(WhereABoardCouldBe(false), "").is_err());
    assert!(not_move_line(WhereABoardCouldBe(false), "* comment line\n").is_err());
    assert!(not_move_line(
        WhereABoardCouldBe(false),
        "手数----指手---------消費時間--\n"
    )
    .is_ok());
    assert!(not_move_line(WhereABoardCouldBe(false), "1 ７六歩(77) ( 0:16/00:00:16)").is_err());
}

#[test]
fn parse_information_preset() {
    assert!(information_line_preset("").is_err());
    assert_eq!(
        Ok(("", Information::Preset(Preset::PresetHirate))),
        information_line_preset("手合割：平手　　\n")
    );
    assert_eq!(
        Ok(("", Information::Preset(Preset::PresetKY))),
        information_line_preset("手合割：香落ち\n")
    );
    assert_eq!(
        Ok(("", Information::Preset(Preset::PresetOther))),
        information_line_preset("手合割：その他\n")
    );
}

#[test]
fn parse_information_hand() {
    assert!(information_line_hands("").is_err());
    // The line comes back with the hand: a hand needs a board, and the
    // reader that finds there is none is not the one that read this line.
    for (line, hand) in [
        (
            "先手の持駒：金　桂　\n",
            Hand {
                KE: 1,
                KI: 1,
                ..Default::default()
            },
        ),
        (
            "下手の持駒：角　\n",
            Hand {
                KA: 1,
                ..Default::default()
            },
        ),
    ] {
        assert_eq!(
            Ok(("", Information::HandBlack(hand, line))),
            information_line_hands(line)
        );
    }
    for (line, hand) in [
        (
            "後手の持駒：角　金三　銀二　桂三　香二　歩十五　\n",
            Hand {
                FU: 15,
                KY: 2,
                KE: 3,
                GI: 2,
                KI: 3,
                KA: 1,
                HI: 0,
            },
        ),
        (
            "後手の持駒：金　桂　香三　歩十　\n",
            Hand {
                FU: 10,
                KY: 3,
                KE: 1,
                GI: 0,
                KI: 1,
                KA: 0,
                HI: 0,
            },
        ),
    ] {
        assert_eq!(
            Ok(("", Information::HandWhite(hand, line))),
            information_line_hands(line)
        );
    }
}

/// A reader that finds no line of its own in any header value — what the
/// KIF side passes, and what these cases are about.
const NOTHING: LineShapes = LineShapes {
    carries_a_line: |_| false,
    opens_line: opens_shared_line,
};

/// One that finds a `▲` enough, for the case that has to be refused.
const ANY_MOVE_MARK: LineShapes = LineShapes {
    carries_a_line: |value| value.contains('▲'),
    opens_line: opens_shared_line,
};

// What separates a line that lost its ending from a line that trails off
// into something the formats do not define. The right-hand column is what
// `ends_here` does with it: refuse the record, or skip the rest of the line.
#[test]
fn what_counts_as_the_line_below() {
    use super::super::{ki2, kif};
    for (shapes, tail) in [
        (kif::SHAPES, "   5 ８八銀(79)   ( 0:01/00:00:05)"),
        // The newline arrived as another byte, so the shapes are looked for
        // past it as well.
        (kif::SHAPES, "\u{0}   5 ８八銀(79)"),
        (kif::SHAPES, ",   5 ８八銀(79)"),
        (kif::SHAPES, "\u{0}*コメント"),
        (kif::SHAPES, ",&しおり"),
        (kif::SHAPES, "\u{0}変化：2手"),
        (kif::SHAPES, "x まで82手で先手の勝ち"),
        (ki2::SHAPES, "\u{0}*コメント"),
        (ki2::SHAPES, "\u{0}変化：2手"),
        (kif::SHAPES, "*コメント"),
        (kif::SHAPES, "&しおり"),
        (kif::SHAPES, "# メモ"),
        (kif::SHAPES, "変化：3手"),
        (kif::SHAPES, "まで82手で先手の勝ち"),
        (ki2::SHAPES, " ▲７六歩 △３四歩"),
        (ki2::SHAPES, "△８四歩"),
        // A fork header this reader cannot parse is still one. Reading
        // only the half-width spelling here let `変化：２手` through as an
        // annotation, and the fork under it carried on as the main line.
        (kif::SHAPES, "変化：２手"),
        (ki2::SHAPES, "変化：２手"),
        (kif::SHAPES, "変化： 2手"),
    ] {
        assert!(
            begins_next_line(shapes, tail),
            "{tail:?} is a line of its own"
        );
    }
    for (shapes, tail) in [
        (kif::SHAPES, "+"), // Kifu for Windows marks moves with it
        (kif::SHAPES, "!?"),
        (kif::SHAPES, " 評価値+120"),
        (kif::SHAPES, "( 0:01)"), // a consumed time this reader cannot read
        (kif::SHAPES, "（ 0:01/00:00:01）"),
        (kif::SHAPES, " 55"),
        (kif::SHAPES, "　※好手"),
        (kif::SHAPES, ""),
        // A KIF numbers its move lines, so a `▲` in one is prose.
        (kif::SHAPES, " ※▲２六歩が本筋"),
        (kif::SHAPES, " （▲７六歩まで）"),
        // And what a note opens with is not what a newline turned into.
        (kif::SHAPES, "（まで先手良し）"),
        (kif::SHAPES, "※ 3 分の1"),
        (kif::SHAPES, "[ 3 分]"),
        (kif::SHAPES, "※ 12 手目からの変化"),
        (ki2::SHAPES, "（△８四歩 ▲２六歩）"),
        // And in KI2 a move behind a marker is prose too: what the marker
        // stands in for is a newline, and a note is not one.
        (ki2::SHAPES, " ※△８四歩の変化"),
        (ki2::SHAPES, "（△８四歩）"),
        (ki2::SHAPES, "▲有利"),
        // A newline that is still there was never lost, whatever padding
        // sits before it. Stepping over one reads the line below as this
        // line's overflow and refuses the record over a trailing space.
        (kif::SHAPES, "　\n   2 ３四歩(33)\n"),
        (kif::SHAPES, "\t\n*コメント\n"),
        (kif::SHAPES, " \n変化：2手\n"),
        (ki2::SHAPES, "　\n△８四歩\n"),
        // A note that names the move it is about is prose with a move at
        // the head of it. `まで…` and `変化：` are where D18 puts notes, so
        // a line of KI2 has to be moves the whole way to count as one.
        (ki2::SHAPES, " △８四歩が最善だった"),
        (ki2::SHAPES, "　△３三桂が敗着"),
        (ki2::SHAPES, " △８四歩から"),
        // A fork header that does not own the line it would begin is
        // prose about a fork. `  83 投了    変化：2手  ( 0:00)` is how
        // the annotation column of a move line reads.
        (kif::SHAPES, "    変化：2手     ( 0:00/00:00:00)"),
        (kif::SHAPES, "    変化：２手     ( 0:00/00:00:00)"),
        (kif::SHAPES, " 変化：2手を参照"),
        (ki2::SHAPES, " 変化：２手を参照"),
    ] {
        assert!(
            !begins_next_line(shapes, &format!("{tail}\n")),
            "{tail:?} is an annotation, and dropping it loses nothing"
        );
    }
}

// What a reader counts as a fork header and what it can consume are the
// same set, so that no line falls between them: counted, and so not skipped
// as prose; unreadable, and so reported by the leftover-input check (D1).
#[test]
fn every_fork_header_that_counts_as_one_can_be_read_as_one() {
    for (head, ply) in [
        ("変化：2手", 2),
        ("変化：２手", 2),
        ("変化： 2手", 2),
        ("変化：\t１２手", 12),
        ("変化：38", 38),
        // Real records need three digits; these are here because a damaged
        // one must not be fatal. The number saturates rather than failing,
        // because failing would put the line in the difference between the
        // two questions — which is the thing this pair exists to close.
        ("変化：2000手", 2000),
        ("変化：18446744073709551615手", usize::MAX),
        ("変化：18446744073709551616手", usize::MAX),
        ("変化：99999999999999999999手", usize::MAX),
        // Ply 0 is read here. Refusing it belongs to `ki2::fork_header`,
        // the only reader that uses the number (D3).
        ("変化：0手", 0),
        ("変化：０手", 0),
    ] {
        assert!(opens_fork_header(head), "{head:?}");
        let (rest, read) = fork_header_ply(head).unwrap_or_else(|e| panic!("{head:?}: {e:?}"));
        assert_eq!(ply, read, "{head:?}");
        assert!(rest.is_empty(), "{head:?} left {rest:?}");
    }
    for head in ["変化：ここから", "変化：", "変化：手", "変わり：2手"] {
        assert!(!opens_fork_header(head), "{head:?}");
        assert!(fork_header_ply(head).is_err(), "{head:?}");
    }
}

// The header block is read twice — before the board and after it — and a
// comment can stand in either half (R-KIF-010). Both halves and their order
// are contracts nothing else states: a JKF is a list, and a reader that
// returns the same comments in a different order returns a different
// record. Records with a board are the ones that reach this path at all
// (詰将棋 and any 任意局面, `research/90-gaps.md` GAP-007).
#[test]
fn a_comment_on_either_side_of_the_board_is_kept_in_the_order_it_was_written() {
    use crate::parser::parse_kif_str;
    const EMPTY_BOARD: &str = concat!(
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
        "+---------------------------+\n",
    );
    // `E` is read by the move reader rather than by the header block, so it
    // is what says the two lists are joined the way the file wrote them.
    let jkf = parse_kif_str(&format!(
        "*A\n後手の持駒：歩\n*B\n{EMPTY_BOARD}*C\n先手の持駒：角\n*D\n先手番\n\
             手数----指手---------消費時間--\n*E\n"
    ))
    .expect("reads");
    assert_eq!(
        Some(&vec![
            String::from("A"),
            String::from("B"),
            String::from("C"),
            String::from("D"),
            String::from("E"),
        ]),
        jkf.moves[0].comments.as_ref(),
        "the halves are joined out of order, or one of them is dropped"
    );
    // And the hands stated on either side of the board are both read.
    let data = jkf.initial.expect("a position").data.expect("a board");
    assert_eq!(1, data.hands[0].KA);
    assert_eq!(1, data.hands[1].FU);
}

// A value this crate has no board for still has to land somewhere. The
// preset rule takes either colon and then declines, so without a landing
// place the line falls to the prose skip and the record loses even the word
// the file wrote — it is kept under `header`. A known name with a note after
// it is the name (GAP-021). A key nobody knows keeps the full-width colon:
// otherwise every `key: value` in any text is a header line (D1 / D8).
#[test]
fn a_known_key_may_use_either_colon_and_still_lands_somewhere() {
    use crate::parser::parse_kif_str;
    const TAIL: &str = "手数----指手---------消費時間--\n";
    for colon in ['：', ':'] {
        let known = parse_kif_str(&format!("手合割{colon}香落ち\n{TAIL}")).expect("reads");
        assert_eq!(Preset::PresetKY, known.initial.expect("a position").preset);

        let noted = parse_kif_str(&format!("手合割{colon}香落ち（30分）\n{TAIL}")).expect("reads");
        assert_eq!(Preset::PresetKY, noted.initial.expect("a position").preset);

        for value in ["詰将棋", "角香交じり"] {
            let jkf = parse_kif_str(&format!("手合割{colon}{value}\n{TAIL}"))
                .unwrap_or_else(|e| panic!("手合割{colon}{value}: {e}"));
            assert_eq!(
                Some(&String::from(value)),
                jkf.header.get("手合割"),
                "手合割{colon}{value}: the line went without a word"
            );
        }
    }
    // A key this reader does not know keeps the full-width colon, so that a
    // file which is not a kifu still says so.
    assert!(parse_kif_str("{\"header\":{},\"moves\":[{}]}\n").is_err());
}

// A line counted as a move line always reaches a reader — either
// `kif::move_line` takes it, or the leftover-input check names it. The two
// sets are deliberately *not* equal: `   2 パス` and `   1 ７六歩(00)` are
// counted and cannot be read, and that is the trade D1 and D8 take
// (GAP-020 of `research/90-gaps.md`). What must not happen is the other
// direction — a line the skip takes that `move_line` would have read.
#[test]
fn a_move_line_a_writer_left_unaligned_is_still_a_move_line() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KIF: &str = concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ７六歩(77)\n   2 ８四歩(83)\n   3 ２六歩(27)\n",
    );
    for first in [
        "   2 ８四歩(83)",
        "   2８四歩(83)",
        "2 ８四歩(83)",
        "2８四歩(83)",
    ] {
        let jkf = parse_kif_str(&format!("{KIF}\n変化：2手\n{first}\n"))
            .unwrap_or_else(|e| panic!("{first:?}: {e}"));
        assert_eq!(
            1,
            jkf.moves.iter().filter(|m| m.forks.is_some()).count(),
            "{first:?}: the fork went"
        );
    }
    // A number followed by something that is neither padding nor a move is
    // prose, and a number followed by padding is a move line whatever comes
    // after — including a word this reader has no meaning for, which the
    // leftover-input check then names (D1, D8, GAP-020).
    for prose in ["　35手目まで", "　1図以下、先手優勢", "  55"] {
        assert!(
            parse_kif_str(&format!("{KIF}{prose}\n")).is_ok(),
            "{prose:?}"
        );
        assert!(
            parse_ki2_str(&format!("手合割：平手\n▲７六歩 △８四歩\n{prose}\n")).is_ok(),
            "{prose:?} in a KI2"
        );
    }
    // A number and then padding is the shape of a move line whatever comes
    // after — the trailing space is not what the line says, it is what the
    // reader has to decide about, and D8 sends it to the leftover-input
    // check rather than the skip (`main` refuses these too).
    for line in ["   2 パス", "   1 ７六歩(00)", "  55 ", "  55　", "   3   "] {
        assert!(
            parse_kif_str(&format!("{KIF}{line}\n")).is_err(),
            "{line:?}"
        );
    }
    // `   2A８四歩(83)` — a number and then neither padding nor a move — is
    // read as prose, as it always has been. Nothing tells it apart from a
    // note that opens with a digit (GAP-020).
    assert!(parse_kif_str(&format!("{KIF}   2A８四歩(83)\n")).is_ok());
}

// The full-width colon is the one the format uses, so a key that holds a
// half-width one is still split at the full-width one. Asking the wider
// question first cuts the key short, and the line then looks like an
// unknown key with a half-width colon and is dropped — including lines this
// crate's own writer produces (R-KIF-004, D4).
#[test]
fn a_key_may_hold_a_half_width_colon_of_its_own() {
    use crate::converter::ToKif;
    use crate::parser::parse_kif_str;
    const TAIL: &str = "手数----指手---------消費時間--\n   1 ７六歩(77)\n";
    for (line, key, value) in [
        ("備考:あり：なし", "備考:あり", "なし"),
        (
            "URL:http://example.com：メモ",
            "URL:http://example.com",
            "メモ",
        ),
        ("13:00：開始", "13:00", "開始"),
        ("備考：a:b", "備考", "a:b"),
    ] {
        let jkf =
            parse_kif_str(&format!("{line}\n{TAIL}")).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        assert_eq!(
            Some(&String::from(value)),
            jkf.header.get(key),
            "{line:?}: {:?}",
            jkf.header
        );
    }
    // And what the writer produces from such a key reads back.
    let jkf: JsonKifuFormat = serde_json::from_str(
        r#"{"header":{"a:b":"c"},"initial":{"preset":"HIRATE"},"moves":[{}]}"#,
    )
    .expect("reads the JKF");
    let written = jkf.try_to_kif_owned().expect("writes KIF");
    assert_eq!(
        Some(&String::from("c")),
        parse_kif_str(&written)
            .unwrap_or_else(|e| panic!("{written:?}: {e}"))
            .header
            .get("a:b")
    );
}

// A text file need not end with a newline (`end_of_line`), and a header
// block can be the whole of one: a position with no moves reaches the reader
// through no other path (GAP-007). Requiring `line_ending` there drops the
// last line and says nothing about it.
#[test]
fn the_header_block_reads_without_a_trailing_newline() {
    use crate::parser::parse_kif_str;
    const BOARD: &str = concat!(
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
        "+---------------------------+\n",
    );
    assert_eq!(
        Some(&String::from("竜王戦")),
        parse_kif_str("手合割：平手\n棋戦：竜王戦")
            .expect("reads")
            .header
            .get("棋戦")
    );
    assert_eq!(
        Preset::PresetKY,
        parse_kif_str("手合割：香落ち")
            .expect("reads")
            .initial
            .expect("a position")
            .preset
    );
    let hand = parse_kif_str(&format!("後手の持駒：飛\n{BOARD}先手の持駒：金"))
        .expect("reads")
        .initial
        .expect("a position")
        .data
        .expect("a board");
    assert_eq!(1, hand.hands[0].KI);
    let side = parse_kif_str(&format!("後手の持駒：飛\n{BOARD}先手の持駒：金\n後手番"))
        .expect("reads")
        .initial
        .expect("a position")
        .data
        .expect("a board");
    assert_eq!(Color::White, side.color);
    // Including a hand line with nothing after the colon. R-KIF-014 asks a
    // *writer* to spell an empty hand `なし`; a reader that refuses the
    // blank one refuses the file over a line that says nothing. A hand needs a
    // board to sit on, so the check is that the line is read — the record
    // itself is refused for having no board (D1).
    let hand_without_a_board =
        parse_kif_str("後手の持駒：飛\n先手の持駒：").expect_err("a hand with no board is refused");
    assert!(
        format!("{hand_without_a_board}").contains("no board"),
        "{hand_without_a_board}"
    );
    // Including the board's own closing frame.
    assert!(
        parse_kif_str(&format!("後手の持駒：飛\n{}", BOARD.trim_end()))
            .expect("reads")
            .initial
            .expect("a position")
            .data
            .is_some()
    );
}

// One contract for every shape the two formats share: the indentation is
// looked past for all of them, or the callers cannot tell which answer they
// are getting.
#[test]
fn a_shared_shape_is_one_however_far_in_it_starts() {
    for pad in ["", " ", "　", "\t"] {
        for line in [
            "*コメント",
            "&しおり",
            "# メモ",
            "変化：2手",
            "まで2手で投了",
        ] {
            assert!(
                opens_shared_line(&format!("{pad}{line}")),
                "{pad:?} + {line:?}"
            );
        }
    }
}

// A board diagram reaches the reader through no other path (GAP-007), so a
// line of one that the readers cannot take must not be taken by the skip
// instead: the record then comes back as an empty 平手 and the writer saves
// that over the original (D4). Padding at the end of a line is padding;
// anything else broken about the diagram is reported (D1).
#[test]
fn a_board_is_read_or_reported_but_never_skipped() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const ROWS: [&str; 12] = [
        "  ９ ８ ７ ６ ５ ４ ３ ２ １",
        "+---------------------------+",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九",
        "+---------------------------+",
    ];
    let record = |rows: &[String]| {
        format!(
            "手合割：詰将棋\n後手の持駒：飛二\n{}先手の持駒：金\n後手番\n",
            rows.concat()
        )
    };
    // Padding at the end of any one line, on any one row.
    for pad in [" ", "　", "\t"] {
        for broken in 0..ROWS.len() {
            let rows: Vec<String> = ROWS
                .iter()
                .enumerate()
                .map(|(i, line)| {
                    if i == broken {
                        format!("{line}{pad}\n")
                    } else {
                        format!("{line}\n")
                    }
                })
                .collect();
            let jkf = parse_kif_str(&record(&rows))
                .unwrap_or_else(|e| panic!("{pad:?} on row {broken}: {e}"));
            assert!(
                jkf.initial.expect("a position").data.is_some(),
                "{pad:?} on row {broken}: the board went"
            );
        }
    }
    // And a diagram that is actually broken says so rather than vanishing.
    for (name, broken, line) in [
        ("a short frame", 1, "+--------------------------+\n"),
        ("a half-width rank", 2, "| ・ ・ ・ ・ ・ ・ ・ ・ ・|1\n"),
        ("eight files", 3, "| ・ ・ ・ ・ ・ ・ ・ ・|二\n"),
    ] {
        let rows: Vec<String> = ROWS
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == broken {
                    String::from(line)
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        assert!(parse_kif_str(&record(&rows)).is_err(), "{name}");
        // D18: the same record, and the same answer. A diagram is read or
        // reported, in either format — never skipped, which would leave the
        // record as an empty 平手 (GAP-007, D4).
        let ki2 = format!("{}▲７六歩 △３四歩\n", record(&rows));
        assert!(parse_ki2_str(&ki2).is_err(), "{name}, as a KI2");
    }
    // A record whose diagram is broken and which has no moves at all — a
    // 詰将棋 figure and its hands, and nothing else. Nothing downstream ever
    // reads the diagram again (GAP-007), so a skip that takes its lines
    // takes the position, and `to_ki2` saves 平手 over the original (D4).
    for (name, broken, line) in [
        ("a short frame", 1, "+--------------------------+\n"),
        ("a half-width rank", 2, "| ・ ・ ・ ・ ・ ・ ・ ・ ・|1\n"),
    ] {
        let rows: Vec<String> = ROWS
            .iter()
            .enumerate()
            .map(|(i, l)| {
                if i == broken {
                    String::from(line)
                } else {
                    format!("{l}\n")
                }
            })
            .collect();
        assert!(
            parse_ki2_str(&record(&rows)).is_err(),
            "{name}, as a KI2 with no moves"
        );
        // And with the last game's special move line left above it. A special move
        // is not a move: the diagram below it is still a diagram.
        let with_a_special = format!("まで122手で先手の勝ち\n{}", record(&rows));
        assert!(
            parse_ki2_str(&with_a_special).is_err(),
            "{name}, as a KI2 under a まで line"
        );
        assert!(
            parse_kif_str(&with_a_special).is_err(),
            "{name}, as a KIF under a まで line"
        );
    }
    // Including one that stops in the middle of the file.
    let cut: Vec<String> = ROWS[..6].iter().map(|l| format!("{l}\n")).collect();
    assert!(parse_kif_str(&record(&cut)).is_err(), "a diagram cut short");
    assert!(
        parse_ki2_str(&format!("{}▲７六歩\n", record(&cut))).is_err(),
        "a diagram cut short, as a KI2"
    );

    // Past the first move a `|` or `+` line cannot be a piece of a board —
    // the board is read once, before them — so refusing one there buys
    // nothing and drops records the format has always allowed.
    const PLAYED: &str = concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ７六歩(77)\n   2 ８四歩(83)\n",
    );
    for line in ["|先手|後手|", "+123", "+-+-+", "+7776FU"] {
        assert!(
            parse_kif_str(&format!("{PLAYED}{line}\n")).is_ok(),
            "{line:?} after the moves"
        );
        assert!(
                parse_kif_str(&format!(
                    "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n{line}\n   2 ８四歩(83)\n"
                ))
                .is_ok(),
                "{line:?} between the moves"
            );
        assert!(
            parse_ki2_str(&format!("手合割：平手\n▲７六歩 △８四歩\n{line}\n")).is_ok(),
            "{line:?} in a KI2"
        );
    }
}

// R-KIF-014 asks a *writer* to spell an empty hand `なし`. A reader that
// refuses the blank one refuses the whole file — board and all — over a
// line that says nothing, and tsshogi's `readHand` drops an empty section
// exactly as it drops `なし`.
#[test]
fn an_empty_hand_line_is_an_empty_hand() {
    use crate::parser::parse_kif_str;
    const BOARD: &str = concat!(
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
        "+---------------------------+\n先手の持駒：なし\n先手番\n",
    );
    for (value, fu, ka) in [
        ("なし", 0, 0),
        ("", 0, 0),
        (" ", 0, 0),
        ("　", 0, 0),
        ("\t", 0, 0),
        ("歩", 1, 0),
        ("歩十八", 18, 0),
        ("歩 角", 1, 1),
        // The padding in front of the value belongs to the value too.
        (" なし", 0, 0),
        (" 歩", 1, 0),
        ("　歩", 1, 0),
        ("\t歩 角", 1, 1),
    ] {
        let jkf = parse_kif_str(&format!("後手の持駒：{value}\n{BOARD}"))
            .unwrap_or_else(|e| panic!("{value:?}: {e}"));
        let hand = jkf
            .initial
            .expect("a position")
            .data
            .expect("a board")
            .hands[1];
        assert_eq!((fu, ka), (hand.FU, hand.KA), "{value:?}");
    }
}

// The first line of a run is the one the skip sees before any reader does,
// so what counts as a move line there has to be what `kif::move_line` will
// consume — including the `( 0:03/00:00:03)` a KIF writes after a special move
// (R-KIF-007). Counting it as prose skips the line, and the game comes back
// `Ok` with no special move at all (D1).
#[test]
fn a_special_at_the_head_of_a_run_keeps_its_clock() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n";
    for (line, special) in [
        ("1投了 ( 0:03/00:00:03)", Some(MoveSpecial::SpecialToryo)),
        ("1投了", Some(MoveSpecial::SpecialToryo)),
        ("1中断 ( 0:00/00:00:00)", Some(MoveSpecial::SpecialChudan)),
        (
            "1千日手 ( 0:00/00:00:00)",
            Some(MoveSpecial::SpecialSennichite),
        ),
        (
            "   1 投了 ( 0:03/00:00:03)",
            Some(MoveSpecial::SpecialToryo),
        ),
        // What may follow the word is the line-end rule's answer and
        // nobody else's (D17): `ends_here` takes a note the same way it
        // takes one after a move. A second answer here is what made
        // `1投了 ( 0:03)` and `1投了+` vanish while `   1 投了 …` read.
        ("1投了もあった", Some(MoveSpecial::SpecialToryo)),
        ("1投了+", Some(MoveSpecial::SpecialToryo)),
        ("1投了 ( 0:03)", Some(MoveSpecial::SpecialToryo)),
        ("1中断 （封じ手）", Some(MoveSpecial::SpecialChudan)),
    ] {
        let jkf =
            parse_kif_str(&format!("{HEAD}{line}\n")).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        assert_eq!(
            special,
            jkf.moves.last().and_then(|mf| mf.special),
            "{line:?}"
        );
    }
    // D18: a `変化：` block whose only line is a special move is a fork, not
    // an empty block.
    let forked = parse_kif_str(concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ７六歩(77)\n   2 ８四歩(83)\n\n変化：2手\n2投了 ( 0:03/00:00:03)\n"
    ))
    .expect("reads");
    assert_eq!(
        1,
        forked.moves.iter().filter(|mf| mf.forks.is_some()).count()
    );
    // And a KIF move line that ends up in a `.ki2` is reported, never
    // skipped — the special move would go with it.
    assert!(parse_ki2_str("手合割：平手\n▲７六歩 △８四歩\n3投了 ( 0:03/00:00:03)\n").is_err());
}

// A move spelled the way KIF does not spell it (R-KIF-006 forbids 不成) is
// still a move line: other software writes it, and a reader that counts only
// what a KIF writer should produce hands it to the skip, which drops the
// move without a word (D1). What separates it from a note that quotes a move
// is that a move says what happened to the piece.
#[test]
fn a_move_line_written_the_wrong_way_is_reported_not_dropped() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KIF: &str =
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
    const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
    // R-KIF-006 tells a *writer* not to spell 不成. The spelling is
    // unambiguous, so refusing to read it loses a record over a word this
    // reader understands (R-REQ-004 / D12).
    let read = parse_kif_str(&format!("{KIF}3２二角不成(88)\n")).expect("不成 reads");
    assert_eq!(
        Some(false),
        read.moves[3].move_.as_ref().and_then(|mv| mv.promote),
        "不成 says the move did not promote"
    );
    // A move whose origin cannot be read is reported in both formats, as it
    // is when the writer did align it.
    for line in ["3２二角成(８８)", "3２二角不成", "3２二角成"] {
        assert!(
            parse_kif_str(&format!("{KIF}{line}\n")).is_err(),
            "{line:?}"
        );
        assert!(
            parse_kif_str(&format!("{KIF}   3 {}\n", &line[1..])).is_err(),
            "{line:?}, aligned"
        );
        assert!(
            parse_ki2_str(&format!("{KI2}{line}\n")).is_err(),
            "{line:?}"
        );
    }
    // A note that quotes a move, written in the shape of one, is refused
    // with it. `1同歩` and `2７六歩` — a note and a move line whose origin
    // went missing — are the same characters, so one of the two has to give,
    // and D1 gives the note: a refused note is loud, a dropped move is not
    // (GAP-020 records the trade, and `main` refuses these too).
    for note in ["2同銀と取れば", "2２六歩が本筋", "1同歩", "12３四歩は疑問"] {
        assert!(
            parse_kif_str(&format!("{KIF}{note}\n")).is_err(),
            "{note:?}"
        );
        assert!(
            parse_ki2_str(&format!("{KI2}{note}\n")).is_err(),
            "{note:?}"
        );
    }
    // A note that does not open with a move is still a note.
    for note in ["35手目まで", "　1図以下、先手優勢", "  55", "1図以下"] {
        assert!(parse_kif_str(&format!("{KIF}{note}\n")).is_ok(), "{note:?}");
    }
}

// R-KIF-005: `[<手番>]<移動先座標><駒>…` — the mark is optional and this
// reader ignores it, taking the side from the ply (R-KIF-007). tsshogi reads
// such a line, so a file that opens on the consumer's TS side has to open
// here; refusing the mark refused the record whole.
#[test]
fn a_kif_move_line_may_carry_the_mark_the_format_allows() {
    use crate::parser::parse_kif_str;
    let marked = parse_kif_str(concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ▲７六歩(77)\n   2 △３四歩(33)\n"
    ))
    .expect("a marked record reads");
    let plain = parse_kif_str(concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ７六歩(77)\n   2 ３四歩(33)\n"
    ))
    .expect("reads");
    assert_eq!(plain, marked);
    // Including where the writer left no padding after the number, which is
    // where the skip decides whether the line holds a move at all.
    assert!(parse_kif_str("手合割：平手\n手数----指手---------消費時間--\n1▲７六歩(77)\n").is_ok());
    // The mark says nothing about the side. Marked the wrong way round, the
    // record still reads as the plies say it does.
    let backwards = parse_kif_str(concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 △７六歩(77)\n   2 ▲３四歩(33)\n"
    ))
    .expect("reads");
    assert_eq!(plain, backwards);
}

// A digit at the head of a line is a KIF move line only when a move follows
// it. `1図以下、先手優勢` and `35手目まで` are how a KIF annotates itself,
// and refusing to skip them refuses the record over a note (D17). The same
// question keeps a KIF move line that ended up in a `.ki2` from being
// skipped, which would take the move with it (D1).
#[test]
fn a_digit_opens_a_move_line_only_when_a_move_follows_it() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KIF: &str =
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
    const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
    for note in [
        "　1図以下、先手優勢",
        "　3手目は疑問",
        "\t35手目まで",
        " 1図以下",
        "55",
    ] {
        // Asked of the rest of the input, so the line it is about has to
        // end somewhere: a file that stops mid-line is a record cut short,
        // not a line the reader has no shape for.
        assert!(
            !opens_numbered_line(&format!("{}\n", note.trim_start_matches(is_padding))),
            "{note:?}"
        );
        let kif = parse_kif_str(&format!("{KIF}{note}\n"))
            .unwrap_or_else(|e| panic!("{note:?} in a KIF: {e}"));
        assert_eq!(2, kif.moves.len() - 1, "{note:?}");
        let ki2 = parse_ki2_str(&format!("{KI2}{note}\n"))
            .unwrap_or_else(|e| panic!("{note:?} in a KI2: {e}"));
        assert_eq!(2, ki2.moves.len() - 1, "{note:?}");
    }
    // And a numbered line that does carry a move is never skipped, in
    // either format.
    assert!(parse_ki2_str(&format!("{KI2}   3 ２六歩(27)\n")).is_err());
}

// A mark at the head of a line is how a move opens, and the indentation in
// front of it does not change that — `　▲有利でした` is refused exactly as
// `▲有利でした` is. A mark behind a note marker (`※▲２六歩が本筋`) is a note
// in both readers (GAP-029).
#[test]
fn a_mark_opens_a_line_from_wherever_the_line_starts() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KIF: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
    for pad in ["", " ", "\t", "　", "\u{a0}"] {
        assert!(
            parse_kif_str(&format!("{KIF}{pad}▲有利でした\n")).is_err(),
            "{pad:?}: a line opening with a mark was skipped"
        );
        assert!(
            parse_ki2_str(&format!("手合割：平手\n▲７六歩\n{pad}△有利でした\n")).is_err(),
            "{pad:?}"
        );
        // A note that only quotes a mark is still prose in KIF (GAP-029).
        assert!(
            parse_kif_str(&format!("{KIF}{pad}※▲２六歩が本筋\n")).is_ok(),
            "{pad:?}"
        );
    }
}

// A `変化：<数字>` line is a fork declaration whatever follows the number
// (D20). What the suffix decides is whether an empty block under it is a
// fork that went missing (D1) or a note that never had one.
#[test]
fn a_fork_header_is_the_whole_of_what_its_number_says() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KIF: &str =
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
    const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
    // A note about a fork, with nothing under it: skipped, in both
    // formats.
    for note in [
        "変化：2手を参照",
        "変化：2手が有力",
        "変化：2手目以下は別途",
        "変化：ここから",
    ] {
        assert!(!is_reportable_empty_block(note), "{note:?}");
        let kif = parse_kif_str(&format!("{KIF}{note}\n"))
            .unwrap_or_else(|e| panic!("{note:?} in a KIF: {e}"));
        assert_eq!(2, kif.moves.len() - 1, "{note:?}");
        let ki2 = parse_ki2_str(&format!("{KI2}{note}\n"))
            .unwrap_or_else(|e| panic!("{note:?} in a KI2: {e}"));
        assert_eq!(2, ki2.moves.len() - 1, "{note:?}");
    }
    // And with a block under it, every one of those is the fork it says
    // it is — the suffix is a note on the header, not a different kind of
    // line. Reading it as prose lets the fork run on as the main line,
    // which changes whose game it is (R-JKF-004).
    for header in [
        "変化：2手",
        "変化：2手 別案",
        "変化：2手（本命）",
        "変化：2",
        "変化：2手目",
        "変化：2手。",
        "変化：2手/A",
    ] {
        assert!(opens_fork_header(header), "{header:?}");
        let jkf = parse_kif_str(&format!("{KIF}\n{header}\n   2 ３四歩(33)\n"))
            .unwrap_or_else(|e| panic!("{header:?}: {e}"));
        assert_eq!(
            1,
            jkf.moves.iter().filter(|m| m.forks.is_some()).count(),
            "{header:?}"
        );
        let ki2 = parse_ki2_str(&format!("{KI2}{header}\n△８四歩\n"))
            .unwrap_or_else(|e| panic!("{header:?} in a KI2: {e}"));
        assert_eq!(
            1,
            ki2.moves.iter().filter(|m| m.forks.is_some()).count(),
            "{header:?} in a KI2"
        );
    }
    // A header that says nothing but its number, with nothing under it, is
    // a fork that went missing and still says so (D1).
    assert!(parse_kif_str(&format!("{KIF}\n変化：2手\n")).is_err());
    assert!(parse_ki2_str(&format!("{KI2}変化：2手\n")).is_err());
}

// Every line the header block is made of, at every indentation. The skip
// and the readers have to widen together: a line the readers can no longer
// take is taken by the skip, and the record comes back `Ok` without it —
// a tsume whose board is gone reads as an empty 平手, and a handicap that
// fell into `header` reverses every move's side (R-HC-001 / R-RULE-006).
//
// The board is the reason this matters at all: a record with one reaches
// the reader through no other path (`research/90-gaps.md` GAP-007).
#[test]
fn the_header_block_reads_at_any_indentation() {
    use crate::parser::parse_kif_str;
    const BOARD: &str = concat!(
        "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
        "+---------------------------+\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
        "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
        "+---------------------------+\n",
    );
    for pad in ["", " ", "   ", "\t", "　", "\u{a0}"] {
        let indented = BOARD
            .lines()
            .map(|line| format!("{pad}{line}\n"))
            .collect::<String>();
        let jkf = parse_kif_str(&format!(
            "{indented}{pad}後手の持駒：飛\n{pad}先手の持駒：なし\n{pad}後手番\n"
        ))
        .unwrap_or_else(|e| panic!("{pad:?} on a board: {e}"));
        let data = jkf
            .initial
            .expect("a position")
            .data
            .unwrap_or_else(|| panic!("{pad:?}: the board went"));
        assert_eq!(Color::White, data.color, "{pad:?}: the side to move went");
        assert_eq!(1, data.hands[1].HI, "{pad:?}: the hand went");

        let handicap = parse_kif_str(&format!(
            "{pad}手合割：香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        ))
        .unwrap_or_else(|e| panic!("{pad:?} on 手合割: {e}"));
        assert_eq!(
            Preset::PresetKY,
            handicap.initial.expect("a position").preset,
            "{pad:?}: the handicap went, and every side with it"
        );

        let header = parse_kif_str(&format!(
            "{pad}棋戦：竜王戦\n手数----指手---------消費時間--\n   1 ７六歩(77)\n"
        ))
        .unwrap_or_else(|e| panic!("{pad:?} on a header: {e}"));
        assert_eq!(
            Some(&String::from("竜王戦")),
            header.header.get("棋戦"),
            "{pad:?}: the indentation went into the key: {:?}",
            header.header
        );

        // The padding on the *other* side of the key belongs to the line
        // too. A `手合割` filed under `header["手合割 "]` is a handicap the
        // board never sees, and D16 misses it as well.
        let before_colon = parse_kif_str(&format!(
            "手合割{pad}：香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        ))
        .unwrap_or_else(|e| panic!("手合割{pad:?}：: {e}"));
        assert_eq!(
            Preset::PresetKY,
            before_colon.initial.expect("a position").preset,
            "手合割{pad:?}："
        );
        // And on the value's side of the colon. A handicap that falls to
        // the key-value rule leaves the board at 平手, where Black opens and
        // the upper hand does not (R-HC-001 / R-RULE-006).
        let after_colon = parse_kif_str(&format!(
            "手合割：{pad}香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        ))
        .unwrap_or_else(|e| panic!("手合割：{pad:?}香落ち: {e}"));
        assert_eq!(
            Preset::PresetKY,
            after_colon.initial.expect("a position").preset,
            "手合割：{pad:?}香落ち"
        );
        assert_eq!(
            Color::White,
            after_colon.moves[1].move_.expect("a move").color,
            "手合割：{pad:?}香落ち: the handicap went, and the sides with it"
        );
        assert!(
            crate::handicap::is_known_name(&format!("{pad}香落ち")),
            "{pad:?}: the writer would drop the preset line the reader read (D16)"
        );
        let key_padded = parse_kif_str(&format!(
            "棋戦{pad}：竜王戦\n手数----指手---------消費時間--\n   1 ７六歩(77)\n"
        ))
        .unwrap_or_else(|e| panic!("棋戦{pad:?}：: {e}"));
        assert_eq!(
            Some(&String::from("竜王戦")),
            key_padded.header.get("棋戦"),
            "棋戦{pad:?}：: {:?}",
            key_padded.header
        );
    }
}

// R-KIF-014 / D5: tsshogi matches `[：:]` where a keyword names the line, so
// a record it opens has to open here. Not on the generic key-value line,
// which takes anything as a key — see `colon`.
#[test]
fn a_keyword_names_its_line_with_either_colon() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    for colon in ['：', ':'] {
        let kif = parse_kif_str(&format!(
            "手合割{colon}香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        ))
        .unwrap_or_else(|e| panic!("手合割{colon}: {e}"));
        assert_eq!(Preset::PresetKY, kif.initial.expect("a position").preset);
        assert_eq!(Color::White, kif.moves[1].move_.expect("a move").color);

        let ki2 = parse_ki2_str(&format!(
            "手合割{colon}平手\n▲７六歩 △８四歩 ▲２六歩\n変化{colon}3手\n▲２五歩\n"
        ))
        .unwrap_or_else(|e| panic!("変化{colon}: {e}"));
        assert!(
            ki2.moves[3].forks.is_some(),
            "変化{colon}: the fork read as the main line carrying on: {:?}",
            ki2.moves
        );
    }
    // And a file that is not a kifu stays one. A half-width colon on the
    // generic key-value line would make a header out of every `key: value`.
    assert!(parse_kif_str("{\"header\":{},\"moves\":[{}]}\n").is_err());
}

// The padding after a value belongs to the value. R-KIF-014 is the shape
// the consumer's TS side also reads (tsshogi splits the hand on spaces and
// drops the empty pieces), so a record it opens has to open here too — and
// a `手合割` left to the key-value rule is a handicap the board never sees,
// which reads back as 平手 with every side reversed (R-HC-001 / R-RULE-006).
#[test]
fn padding_after_a_header_value_belongs_to_the_value() {
    use crate::parser::parse_kif_str;
    const BOARD: &str = "  ９ ８ ７ ６ ５ ４ ３ ２ １\n\
            +---------------------------+\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n+---------------------------+\n\
            先手の持駒：なし\n先手番\n手数----指手---------消費時間--\n";
    for pad in ["", " ", "　", "\t", "\u{a0}"] {
        for hand in ["なし", "歩", "歩二 角"] {
            let jkf = parse_kif_str(&format!("後手の持駒：{hand}{pad}\n{BOARD}"))
                .unwrap_or_else(|e| panic!("{hand:?} + {pad:?}: {e}"));
            assert!(jkf.initial.expect("a position").data.is_some());
        }
        let jkf = parse_kif_str(&format!(
            "手合割：香落ち{pad}\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        ))
        .unwrap_or_else(|e| panic!("手合割 + {pad:?}: {e}"));
        assert_eq!(Preset::PresetKY, jkf.initial.expect("a position").preset);
        assert_eq!(
            Color::White,
            jkf.moves[1].move_.expect("a move").color,
            "{pad:?}: the handicap went, and the sides with it"
        );
    }
}

#[test]
fn parse_information_keyvalue() {
    assert!(information_line_keyvalue(NOTHING)("").is_err());
    assert!(information_line_keyvalue(NOTHING)("# comment\n").is_err());
    // A text file need not end with a newline, and a header block can be the
    // whole of one (a position with no moves). Requiring `line_ending` drops
    // the last line and says nothing about it — `end_of_line`'s rule.
    assert_eq!(
        Ok((
            "",
            Information::KeyValue(String::from("key"), String::from("value at the end"))
        )),
        information_line_keyvalue(NOTHING)("key：value at the end")
    );
    assert_eq!(
        Ok((
            "",
            Information::KeyValue(String::from("key"), String::from("value"))
        )),
        information_line_keyvalue(NOTHING)("key：value\n")
    );
    // And a value the reader does find one in: the line under it was read as
    // part of it, so the record is short by however much that line held.
    assert!(information_line_keyvalue(ANY_MOVE_MARK)("手合割：平手 ▲７六歩\n").is_err());
    // R-KIF-010 / R-KIF-011: a comment and a bookmark hold free text, and
    // free text holds `：`. Filed as a header they are gone from the record
    // and a key nobody wrote is in it.
    assert!(information_line_keyvalue(NOTHING)("*（主催：新聞三社連合）\n").is_err());
    assert!(information_line_keyvalue(NOTHING)("&しおり：ここ\n").is_err());
}

// R-KIF-010: comments before the first move belong to the initial position.
// KIF ends its header block with `手数----指手---------消費時間--`, which
// R-KIF-012 says a record need not have and KI2 has no equivalent of — so a
// header rule that eats `*` lines eats them out of every KI2 and out of any
// KIF written without that line.
#[test]
fn a_comment_over_the_first_move_is_not_a_header() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    let ki2 =
        parse_ki2_str("手合割：平手\n*（主催：新聞三社連合）\n▲７六歩 △３四歩\n").expect("reads");
    assert_eq!(
        Some(&vec![String::from("（主催：新聞三社連合）")]),
        ki2.moves[0].comments.as_ref(),
        "header: {:?}",
        ki2.header
    );
    assert!(ki2.header.is_empty(), "{:?}", ki2.header);
    let kif =
        parse_kif_str("手合割：平手\n*（主催：新聞三社連合）\n   1 ７六歩(77)\n").expect("reads");
    assert_eq!(ki2.moves[0].comments, kif.moves[0].comments);
    assert!(kif.header.is_empty(), "{:?}", kif.header);
}

// And it does not end the header block either. A rule that refuses the line
// instead of reading it stops `many0`, and everything under the comment —
// the rest of the header, the board, the `手合割` — is read by nobody. A
// handicap lost that way defaults to 平手, and 平手 has Black moving first
// where the record has White (R-HC-001 / R-RULE-006): every side flips.
#[test]
fn a_comment_among_the_header_lines_does_not_end_the_header() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    for opener in ["*メモ：あ", "&しおり：あ", "# Kifu for Windows"] {
        let kif = parse_kif_str(&format!(
            "先手：山田\n{opener}\n後手：田中\n手合割：香落ち\n\
                 手数----指手---------消費時間--\n   1 ３四歩(33)\n"
        ))
        .unwrap_or_else(|e| panic!("{opener}: {e}"));
        assert_eq!(
            Some(&String::from("田中")),
            kif.header.get("後手"),
            "{opener}: the header under the comment is gone: {:?}",
            kif.header
        );
        let initial = kif.initial.as_ref().expect("an initial position");
        assert_eq!(Preset::PresetKY, initial.preset, "{opener}");
        assert_eq!(
            Color::White,
            kif.moves[1].move_.as_ref().expect("a move").color,
            "{opener}: the handicap went, and the sides with it"
        );
    }
    // KI2 has no `手数----` line to end the block with (R-KI2-002), so this
    // is the only thing standing between a note in its header and the rest
    // of the record.
    let ki2 = parse_ki2_str("開始日時：2021\n*（主催：…）\n先手：藤井\n後手：豊島\n▲７六歩\n")
        .expect("reads");
    assert_eq!(3, ki2.header.len(), "{:?}", ki2.header);
}

#[test]
fn parse_informations() {
    assert_eq!(
        Ok(("", InformationData::default())),
        informations(NOTHING, "")
    );
    assert_eq!(
        Ok((
            "",
            InformationData {
                map: [(String::from("key"), String::from("value"))]
                    .into_iter()
                    .collect(),
                ..Default::default()
            }
        )),
        informations(NOTHING, "# comment\n# comment：comment\nkey：value\n")
    );
}

// R-NOT-006: a writer uses the standard form only, and both writers are
// writing the same notation — so one table, and every entry in it has to be
// one the reader takes back. The reader takes the variants too (R-KI2-005),
// so a table that drifted still produces readable files; only a round trip
// that checks *which* piece comes back says the two writers disagree.
#[test]
fn every_move_word_reads_back_as_the_piece_it_names() {
    for kind in [
        Kind::FU,
        Kind::KY,
        Kind::KE,
        Kind::GI,
        Kind::KI,
        Kind::KA,
        Kind::HI,
        Kind::OU,
        Kind::TO,
        Kind::NY,
        Kind::NK,
        Kind::NG,
        Kind::UM,
        Kind::RY,
    ] {
        let word = crate::notation::move_word(kind);
        let (rest, read) =
            piece_kind(word).unwrap_or_else(|e| panic!("{kind:?} spelled {word:?}: {e:?}"));
        assert_eq!(kind, read, "{word:?}");
        assert!(rest.is_empty(), "{word:?} left {rest:?}");
    }
}

#[test]
fn parse_piece_kind() {
    assert!(piece_kind("").is_err());
    assert_eq!(Ok(("", Kind::FU)), piece_kind("歩"));
    assert_eq!(Ok(("", Kind::OU)), piece_kind("玉"));
    assert_eq!(Ok(("", Kind::OU)), piece_kind("王"));
    assert_eq!(Ok(("", Kind::RY)), piece_kind("龍"));
    assert_eq!(Ok(("", Kind::RY)), piece_kind("竜"));
    assert_eq!(Ok(("", Kind::NY)), piece_kind("成香"));
    assert_eq!(Ok(("", Kind::NK)), piece_kind("成桂"));
    assert_eq!(Ok(("", Kind::NG)), piece_kind("成銀"));
    assert_eq!(Ok(("", Kind::NY)), piece_kind("杏"));
    assert_eq!(Ok(("", Kind::NK)), piece_kind("圭"));
    assert_eq!(Ok(("", Kind::NG)), piece_kind("全"));
}

#[test]
fn parse_board_piece() {
    assert!(board_piece("").is_err());
    assert_eq!(Ok(("", Piece::empty())), board_piece(" ・"));
    assert_eq!(
        Ok((
            "",
            Piece {
                color: Some(Color::Black),
                kind: Some(Kind::FU)
            }
        )),
        board_piece(" 歩")
    );
    assert_eq!(
        Ok((
            "",
            Piece {
                color: Some(Color::White),
                kind: Some(Kind::FU)
            }
        )),
        board_piece("v歩")
    );
}

#[test]
fn parse_board_row() {
    let rows = (0..9)
        .map(|i| (0..9).map(|j| HIRATE_BOARD[8 - j][i]).collect::<Vec<_>>())
        .collect::<Vec<_>>();
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[0].clone())), board_row("|v香v桂v銀v金v玉v金v銀v桂v香|一\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[1].clone())), board_row("| ・v飛 ・ ・ ・ ・ ・v角 ・|二\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[2].clone())), board_row("|v歩v歩v歩v歩v歩v歩v歩v歩v歩|三\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[3].clone())), board_row("| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[4].clone())), board_row("| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[5].clone())), board_row("| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[6].clone())), board_row("| 歩 歩 歩 歩 歩 歩 歩 歩 歩|七\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[7].clone())), board_row("| ・ 角 ・ ・ ・ ・ ・ 飛 ・|八\n"));
    #[rustfmt::skip]
        assert_eq!(Ok(("", rows[8].clone())), board_row("| 香 桂 銀 金 玉 金 銀 桂 香|九\n"));
}

#[test]
fn parse_board() {
    assert_eq!(
        Ok(("", HIRATE_BOARD)),
        board(
            &r#"
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
|v香v桂v銀v金v玉v金v銀v桂v香|一
| ・v飛 ・ ・ ・ ・ ・v角 ・|二
|v歩v歩v歩v歩v歩v歩v歩v歩v歩|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| 歩 歩 歩 歩 歩 歩 歩 歩 歩|七
| ・ 角 ・ ・ ・ ・ ・ 飛 ・|八
| 香 桂 銀 金 玉 金 銀 桂 香|九
+---------------------------+
"#[1..]
        )
    );
}
