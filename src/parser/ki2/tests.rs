// The same record read as `.kif` and as `.ki2` is the same record (D18).
// That includes where a comment under the special move belongs: on the
// special move (R-KIF-010), in both readers. A node of its own in only one
// of them gives a game saved as `.ki2` and opened again a node the KIF does
// not have, and the consumer's pointer into the tree moves with it.
#[test]
fn a_comment_under_a_special_belongs_to_the_special() {
    use crate::converter::ToKi2;
    use crate::parser::{parse_ki2_str, parse_kif_str};
    let kif = parse_kif_str(concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ７六歩(77)\n   2 ３四歩(33)\n   3 投了\n*感想戦のメモ\n"
    ))
    .expect("reads");
    assert_eq!(
        Some(&vec![String::from("感想戦のメモ")]),
        kif.moves[3].comments.as_ref()
    );
    let ki2 = kif.try_to_ki2_owned().expect("writes");
    assert_eq!(kif, parse_ki2_str(&ki2).expect("reads back"), "{ki2}");
}

// Every spelling the writer writes, the reader reads back as the same thing
// (R-NOT-004). Two tables of these 13 words is how a file this crate wrote
// stops opening. Nothing else in the suite reaches 左引 / 右引, so this is
// the only test that tells the two words apart.
#[test]
fn every_relative_word_reads_back_as_itself() {
    for relative in Relative::ALL {
        let word = crate::notation::relative_word(relative);
        assert_eq!(
            Ok(("", relative)),
            relative_word_here(word),
            "{relative:?} is written {word:?}"
        );
    }
    // Longest first, or `左上` is taken as `左` with `上` left over.
    for relative in Relative::ALL {
        let word = crate::notation::relative_word(relative);
        let (rest, read) = relative_word_here(word).expect("reads");
        assert!(rest.is_empty(), "{relative:?} left {rest:?}");
        assert_eq!(relative, read);
    }
    // The list is the enum's, in full.
    for (i, relative) in Relative::ALL.into_iter().enumerate() {
        assert_eq!(i, relative.ordinal(), "{relative:?} is out of place");
    }
    // The words themselves, from R-NOT-004. One table cannot drift from
    // itself, so what is left to get wrong is the spelling, and nothing else
    // in the suite reaches 左引 / 右引.
    assert_eq!(
        [
            "左上", "左寄", "左引", "右上", "右寄", "右引", "左", "直", "右", "上", "寄", "引",
            "打",
        ],
        Relative::ALL.map(crate::notation::relative_word)
    );
    // And no two of them are spelled the same.
    for a in Relative::ALL {
        for b in Relative::ALL {
            assert!(
                a == b || crate::notation::relative_word(a) != crate::notation::relative_word(b),
                "{a:?} and {b:?} are spelled the same"
            );
        }
    }
}
use super::*;
use crate::normalizer::ORIGIN_UNSTATED;
use std::collections::HashMap;

// R-NOT-001: <到達地点><駒種><相対位置><動作><成/不成>. Reading the promotion
// suffix before the relative part leaves it unconsumed, which ends the move
// list and drops the rest of the file without an error.
#[test]
fn single_move_reads_relative_before_promotion() {
    for (input, relative, promote) in [
        ("▲５四角右成", Some(Relative::R), Some(true)),
        ("▲５四角左不成", Some(Relative::L), Some(false)),
        ("▲３三銀右上成", Some(Relative::RU), Some(true)),
        ("▲５四角成", None, Some(true)),
        ("▲５四角右", Some(Relative::R), None),
        ("▲５四角", None, None),
        // R-NOT-006 / R-KI2-005: the spellings print uses.
        ("▲５四角行", Some(Relative::U), None),
        ("▲５四角生", None, Some(false)),
    ] {
        let (rest, mf) = single_move(input).unwrap_or_else(|e| panic!("{input}: {e:?}"));
        assert_eq!("", rest, "unconsumed input for {input}");
        let mv = mf.move_.expect("a move");
        assert_eq!(relative, mv.relative, "relative for {input}");
        assert_eq!(promote, mv.promote, "promote for {input}");
    }
}

// The example printed in the KI2 specification itself (R-KI2-002), copied
// verbatim. It has blank lines between runs of moves, which is exactly what
// the specification points out about the format. Stopping at the first one
// reads 6 of the 12 moves and returns `Ok`.
#[test]
fn the_specification_example_reads_every_move() {
    let ki2 = "開始日時：1999/04/08
終了日時：1999/04/09
棋戦：第５７期名人戦７番勝負 第１局
戦型：横歩取り
先手：谷川浩司 九段
後手：佐藤康光 名人

▲７六歩 △３四歩 ▲２六歩 △８四歩 ▲２五歩
△８五歩

▲７八金 △３二金 ▲２四歩 △同　歩 ▲同　飛
△８六歩
";
    let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
    let moves = jkf.moves[1..]
        .iter()
        .filter(|mf| mf.move_.is_some())
        .count();
    assert_eq!(12, moves, "read {moves} of 12: {:?}", jkf.moves);
}

// R-KI2-001: KI2 is "a game record people can read", and the specification
// says a run of moves pasted out of one is readable too. Pasted text arrives
// indented, and stopping at the indent drops every move after it.
#[test]
fn an_indented_run_of_moves_is_read() {
    let ki2 = "▲７六歩 △３四歩\n  ▲２六歩 △８四歩\n";
    let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
    let moves = jkf.moves[1..]
        .iter()
        .filter(|mf| mf.move_.is_some())
        .count();
    assert_eq!(4, moves, "read {moves} of 4: {:?}", jkf.moves);
}

// A comment can open a `変化：` block — that is what this crate's own writer
// produces for a JKF fork whose first node carries only comments. Failing
// to read it ends the move list and throws away every block after it.
#[test]
fn a_fork_may_open_with_a_comment() {
    let ki2 = "▲７六歩 △３四歩

変化：2手
*この分岐の狙い
△８四歩

変化：2手
△４四歩
";
    let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
    let forks = jkf.moves[2].forks.as_ref().expect("forks at ply 2");
    assert_eq!(2, forks.len(), "both blocks survive: {:?}", jkf.moves);
    assert_eq!(
        Some(&vec!["この分岐の狙い".to_owned()]),
        forks[0][0].comments.as_ref(),
        "the comment is kept"
    );
}

// Leaving a `まで…` line unconsumed ends the move list and throws away
// every `変化：` block after it while still returning `Ok`. The special move word
// is the one thing a reader cannot recover from the rest of the file, so a
// record that says something we do not understand has to be reported (D1).
#[test]
fn an_unreadable_special_line_is_an_error_not_a_silent_truncation() {
    for phrase in ["持将棋成立", "先手の不戦敗", "中座", "引き分け"] {
        let ki2 = format!(
            "▲７六歩 △３四歩\nまで2手で{phrase}\n\n変化：2手\n△８四歩\n\n変化：1手\n▲２六歩\n"
        );
        let err = crate::parser::parse_ki2_str(&ki2)
            .err()
            .unwrap_or_else(|| panic!("{phrase} was accepted"));
        assert!(
            matches!(err, crate::error::ParseError::Ki2(_)),
            "{phrase} gave {err:?}"
        );
        // The one thing the reader cannot recover is the word it could not
        // read, so the error has to carry it and point at its own line.
        let text = err.to_string();
        assert!(text.contains(phrase), "{phrase} is missing from {text:?}");
        assert!(
            text.contains("at line 2"),
            "{phrase} should point at line 2: {text:?}"
        );
    }
}

// GAP-017: the ply a special move line names counts moves and special moves, not
// nodes — a comment-only node writes no line for anyone to number. Counting
// nodes instead shifts the special move one ply on, and the side to move with
// it: a bare `反則勝ち` names its winner only through whose turn it is
// (D5), so the record comes back accusing the other player.
//
// A comment-only node is a shape the KI2 *writer* produces for a JKF node
// carrying only comments, and a `変化：` block can open with one — which is
// where the reader meets it, since a comment after a move attaches to that
// move instead. So the round trip is what exercises the count.
#[test]
fn a_comment_only_node_does_not_shift_the_special_ply() {
    use crate::converter::ToKi2;
    use crate::parser::parse_ki2_str;
    const KI2: &str = "手合割：平手
▲７六歩 △３四歩
まで2手で中断

変化：2手
*このあとは
△８四歩
まで2手で反則勝ち
";
    let jkf = parse_ki2_str(KI2).expect("parses");
    let fork = &jkf.moves[2].forks.as_ref().expect("a fork at ply 2")[0];
    assert_eq!(3, fork.len(), "comment node, move, special move: {fork:?}");
    assert_eq!(
        Some(vec![String::from("このあとは")]),
        fork[0].comments,
        "the fork opens with a node that is only a comment"
    );
    // Ply 2 is the fork's move, so the special move takes ply 3 — Black's.
    // R-CSA-007: `+ILLEGAL_ACTION` is Black fouling, so White wins.
    assert_eq!(
        Some(MoveSpecial::SpecialIllegalActionWhite),
        fork[2].special
    );

    // And again through the writer, which is where the shape comes from.
    let written = jkf.try_to_ki2_owned().expect("writes KI2");
    let back = parse_ki2_str(&written).expect("reads back");
    assert_eq!(jkf, back, "{written:?}");
}

// Only prose is skipped. A line this reader has a shape for belongs to
// whoever reads it: a `まで…` is the special move (KI2 has no other way to record
// one, R-KI2-006 / D5), and a `変化：` says a fork starts — including a
// spelling this reader cannot read, which the leftover-input check then
// reports (D1) rather than hiding a fork inside the main line (R-JKF-004).
#[test]
fn a_line_the_reader_has_a_shape_for_is_never_skipped() {
    use crate::parser::parse_ki2_str;
    const MOVES: &str = "手合割：平手\n▲７六歩 △３四歩\n";
    for before in ["# Kifu for Windows V7.30\n", "感想戦\n", "\n"] {
        let jkf = parse_ki2_str(&format!("{MOVES}{before}まで2手で投了\n"))
            .unwrap_or_else(|e| panic!("{before:?}: {e}"));
        assert!(
            jkf.moves.last().expect("a node").special.is_some(),
            "{before:?}: the special move is gone"
        );
    }
    // A fork header is one however its number is spelled, and every
    // spelling `opens_fork_header` counts is one `fork_header_ply`
    // consumes. A spelling only one of the two knows is a line nobody owns:
    // counted, so not skipped as prose; unreadable, so not consumed.
    for header in ["変化：2手", "変化：２手", "変化： 2手", "変化：２手"] {
        for block in ["▲２六歩\n", "まで2手で投了\n"] {
            let jkf = parse_ki2_str(&format!("{MOVES}{header}\n{block}"))
                .unwrap_or_else(|e| panic!("{header:?} + {block:?}: {e}"));
            assert!(
                jkf.moves[2].forks.is_some(),
                "{header:?} + {block:?}: read as the main line carrying on: {:?}",
                jkf.moves
            );
        }
    }
    // And `変化：` with no number is a sentence opening with two characters,
    // so it is prose — the same as KIF makes of it (D18).
    let prose = parse_ki2_str(&format!("{MOVES}変化：ここから\n")).expect("reads");
    assert_eq!(2, prose.moves.len() - 1, "{:?}", prose.moves);
}

// R-KI2-001: KI2 is a record people read, and what people paste is padded
// with whatever the place they read it padded with. A separator narrower
// than `is_padding` leaves the padding inside the special move word, and the
// record comes back as an error naming a word that is in the vocabulary.
// The list is a sample of what `is_padding` answers for, not its definition
// — a keyboard's three, and what a web page and a typesetter put in.
#[test]
fn padding_after_the_special_word_is_padding_whichever_space_it_is() {
    use crate::parser::parse_ki2_str;
    for pad in [' ', '\t', '　', '\u{a0}', '\u{2009}', '\u{b}', '\u{c}'] {
        assert!(super::is_padding(pad), "{pad:?}");
        for src in [
            format!("手合割：平手\n▲７六歩 △３四歩\nまで2手で投了{pad}\n"),
            format!("手合割：平手\n▲７六歩 △３四歩\nまで2手で{pad}投了\n"),
        ] {
            let jkf = parse_ki2_str(&src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
            assert!(
                jkf.moves.last().expect("a node").special.is_some(),
                "{src:?}: the special move is gone"
            );
        }
    }
}

// Plies count from 1 (R-JKF-001), so `変化：０手` names no move for a fork
// to be an alternative to. Read as a number it reaches `attach_fork`,
// which cannot find a departure point and returns — taking the fork and
// the special move under it without a word (GAP-018). Refused, the line is what
// the leftover-input check reports (D1).
//
// KIF has to keep reading it: the tree there comes from the ply numbers on
// the move lines and the declaration is never looked at (D3), so refusing
// would reject a record over a digit KIF does not read.
#[test]
fn a_fork_that_departs_from_no_ply_is_refused_rather_than_dropped() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    const KI2: &str = "手合割：平手\n▲７六歩 △８四歩 ▲２六歩 △４四歩\n";
    const KIF: &str =
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n\n";
    for zero in ["0", "０"] {
        assert!(
            parse_ki2_str(&format!(
                "{KI2}変化：{zero}手\n△８四歩 ▲２五歩\nまで6手で中断\n"
            ))
            .is_err(),
            "変化：{zero}手: the fork and the special move went without a word"
        );
        let kif = parse_kif_str(&format!("{KIF}変化：{zero}手\n   2 ３四歩(33)\n"))
            .unwrap_or_else(|e| panic!("変化：{zero}手 in KIF: {e}"));
        assert_eq!(
            1,
            kif.moves.iter().filter(|m| m.forks.is_some()).count(),
            "変化：{zero}手: KIF builds its tree from the ply numbers (D3)"
        );
    }
    // A ply too large to use is not a spelling that cannot be read: the
    // record still says a fork starts, and D19 puts the ceiling at
    // "a damaged record is not fatal" rather than at supporting the number.
    // The fork itself has no node to hang on, so it goes the way every
    // out-of-range departure ply goes (GAP-018) — that is the part D19
    // accepts, and it is different from `変化：０手`, which is refused
    // because a record cannot mean it.
    for huge in ["18446744073709551616", "99999999999999999999"] {
        let kif =
            parse_kif_str(&format!("{KIF}変化：{huge}手\n   2 ３四歩(33)\n")).unwrap_or_else(|e| {
                panic!("変化：{huge}手: refused over a number KIF never reads: {e}")
            });
        assert_eq!(
            1,
            kif.moves.iter().filter(|m| m.forks.is_some()).count(),
            "変化：{huge}手: KIF builds its tree from the ply numbers (D3)"
        );
        let ki2 = parse_ki2_str(&format!("{KI2}変化：{huge}手\n△８四歩\n"))
            .unwrap_or_else(|e| panic!("変化：{huge}手: {e}"));
        assert_eq!(
            0,
            ki2.moves.iter().filter(|m| m.forks.is_some()).count(),
            "変化：{huge}手: KI2 has no node at that ply, so the fork goes (GAP-018)"
        );
    }
}

// A `変化：` above the first move names no move to be an alternative to.
// tsshogi reads such records deliberately (shogihome #570) and ignores the
// line; KIF has always done the same (`kif::skippable_line_above_the_main_line`). Left in, it
// makes the whole game a fork of a main line with no node to hang it on,
// and `attach_fork` drops it without a word — `Ok`, no moves at all.
#[test]
fn a_fork_header_above_the_first_move_is_read_and_dropped() {
    use crate::parser::parse_ki2_str;
    // The header block holds whatever the writer left between its last
    // keyword and the first move, in whatever order, at whatever column. A
    // gate that fires in one place is a gate the next line walks around.
    for colon in ['：', ':'] {
        for before in [
            "",
            "*まえがき\n",
            "先手番\n",
            "解説\n",
            "*a\n解説\n",
            "解説\n*a\n",
            "# note\n",
            "\n",
        ] {
            for pad in ["", " ", "　", "\t"] {
                let src =
                    format!("先手：A\n{before}{pad}変化{colon}2手\n▲７六歩 △３四歩 ▲２六歩\n");
                let jkf = parse_ki2_str(&src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
                assert_eq!(
                    3,
                    jkf.moves.iter().filter(|m| m.move_.is_some()).count(),
                    "{src:?}: the game was read as a fork and dropped"
                );
            }
        }
    }
    // And the comments among them are still the initial position's, in the
    // order the file wrote them (R-KIF-010).
    let kept = parse_ki2_str("先手：A\n*a\n解説\n*b\n変化：2手\n▲７六歩 △３四歩\n").expect("reads");
    assert_eq!(
        Some(&vec![String::from("a"), String::from("b")]),
        kept.moves[0].comments.as_ref()
    );
    // A fork that does name a move is still a fork, and one with an
    // empty block still says so (D1).
    let jkf = parse_ki2_str("手合割：平手\n▲７六歩 △８四歩 ▲２六歩\n変化：3手\n▲２五歩\n")
        .expect("reads");
    assert!(
        jkf.moves.iter().any(|m| m.forks.is_some()),
        "{:?}",
        jkf.moves
    );
    assert!(parse_ki2_str("手合割：平手\n▲７六歩 △８四歩\n変化：2手\n").is_err());
}

// The padding between two moves is padding, whichever space it is. The two
// questions that call `many1(single_move)` themselves — the ones that catch
// a line whose newline is gone — have to read the same set as the skip in
// `move_run`, or a run separated by a full-width space stops being a run
// halfway through and the rest is taken for a note.
#[test]
fn moves_are_separated_by_padding_whichever_space_it_is() {
    use crate::parser::parse_ki2_str;
    for pad in [" ", "\t", "　", "\u{a0}", "\u{2009}"] {
        assert!(
            parse_ki2_str(&format!("手合割：平手 ▲７六歩{pad}△３四歩\n")).is_err(),
            "{pad:?}: the game went into the header without a word"
        );
        assert!(
            parse_ki2_str(&format!(
                "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断 ▲７六歩{pad}△３四歩\n"
            ))
            .is_err(),
            "{pad:?}: the moves after the special move went without a word"
        );
        let run = parse_ki2_str(&format!("手合割：平手\n▲７六歩{pad}△３四歩\n"))
            .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
        assert_eq!(2, run.moves.len() - 1, "{pad:?}");
    }
}

// R-KI2-001: a run can arrive indented, because KI2 is a record people read
// and paste. The padding in front of a line has to be padding to every
// reader of that line, not just to the ones that ask `is_padding`: a line
// whose shape is only recognised from column 0 falls to the prose skip, and
// what it said goes with it — the special move KI2 has no other place for
// (R-KI2-006 / D5), or a fork whose moves then read as the main line
// carrying on (R-JKF-004).
#[test]
fn a_line_is_the_line_it_is_however_far_in_it_starts() {
    use crate::parser::parse_ki2_str;
    const MOVES: &str = "手合割：平手\n▲７六歩 △３四歩\n";
    for pad in ["", " ", "　", "\t", "\u{a0}"] {
        let special = parse_ki2_str(&format!("{MOVES}{pad}まで2手で投了\n"))
            .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
        assert!(
            special.moves.last().expect("a node").special.is_some(),
            "{pad:?}: the special move is gone"
        );
        let fork = parse_ki2_str(&format!("{MOVES}{pad}変化：2手\n▲２六歩\n"))
            .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
        assert!(
            fork.moves[2].forks.is_some(),
            "{pad:?}: the fork reads as the main line carrying on: {:?}",
            fork.moves
        );
        let run = parse_ki2_str(&format!("手合割：平手\n{pad}▲７六歩 △３四歩\n"))
            .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
        assert_eq!(2, run.moves.len() - 1, "{pad:?}");
    }
}

// D18: `まで<N>手で<結末>` and `変化：N手` are where a note goes, and a note
// naming the move it is about opens with one. Reading a single move as the
// line below refuses records that read as `.kif`; reading a run that fills
// the line as a note lets a lost newline through in silence. Both directions
// in one table.
#[test]
fn a_note_after_a_special_or_a_fork_header_is_a_note() {
    use crate::parser::parse_ki2_str;
    const MOVES: &str = "手合割：平手\n▲７六歩 △３四歩\n";
    for line in [
        "まで2手で投了 △８四歩が最善だった\n",
        "まで2手で投了　△３三桂が敗着\n",
        "まで2手で投了 惜しい将棋だった\n",
    ] {
        let jkf =
            parse_ki2_str(&format!("{MOVES}{line}")).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        assert!(
            jkf.moves.last().expect("a node").special.is_some(),
            "{line:?}: refused, or the special move is gone"
        );
    }
    let jkf = parse_ki2_str("手合割：平手\n▲７六歩 △３四歩\n変化：2手 △８四歩から\n△８四歩\n")
        .expect("a fork header takes a note too");
    assert!(jkf.moves[2].forks.is_some(), "{:?}", jkf.moves);
    // And what fills the line is the line below, whose newline is gone.
    for line in [
        "まで1手で中断 △３四歩\n",
        "まで2手で投了 ▲７六歩 △３四歩\n",
        "まで2手で投了 変化：2手\n△８四歩\n",
        "まで2手で投了 *感想\n",
    ] {
        assert!(
            parse_ki2_str(&format!("{MOVES}{line}")).is_err(),
            "{line:?} was read as a note, and what it held is gone"
        );
    }
}

// A move reads the comments under it as its own, so the last mark in a
// value need not be a move start: a swallowed `*` line puts marks behind
// the run. Asking only the mark before the last one answers no there, and
// the record comes back `Ok` with the game inside `header["手合割"]`.
#[test]
fn a_swallowed_line_is_found_behind_the_comment_it_swallowed() {
    use crate::parser::parse_ki2_str;
    for value in [
        "手合割：平手 ▲７六歩 △３四歩",
        "手合割：平手 ▲７六歩 △３四歩 *よい将棋",
        "手合割：平手 ▲７六歩 △３四歩 *▲２六歩は疑問",
        "手合割：平手 ▲７六歩 △３四歩 &▲２六歩",
    ] {
        assert!(
            parse_ki2_str(&format!("{value}\n▲７六歩 △３四歩\n")).is_err(),
            "{value:?}: the game went into the header without a word"
        );
    }
    // And the values that only quote moves still read (R-KIF-004).
    for value in [
        "消費時間：104▲379△380",
        "note：序盤は▲７六歩 △３四歩 の出だし",
        "戦型：▲居飛車",
    ] {
        let jkf = parse_ki2_str(&format!("{value}\n▲７六歩 △３四歩\n"))
            .unwrap_or_else(|e| panic!("{value:?}: {e}"));
        assert_eq!(2, jkf.moves.len() - 1, "{value:?}");
    }
}

// The value this check exists for is a whole game on one line, and the
// consumer meets it inside a scan of a whole directory at start-up
// (R-REQ-002). Asking the question once per mark made that 19.6 s for 41 KB,
// and the record still came back `Ok` — nothing in a log to explain the
// pause. The bound is loose on purpose: it is here to catch a quadratic
// walk, not to measure the machine.
#[test]
fn a_header_value_holding_a_whole_game_is_read_in_one_pass() {
    use crate::parser::parse_ki2_str;
    let mut src = String::from("手合割：平手 ");
    for i in 0..3200 {
        src.push_str(if i % 2 == 0 {
            "▲７六歩 "
        } else {
            "△３四歩 "
        });
    }
    src.push_str("まで3200手で投了\n");
    let started = std::time::Instant::now();
    let read = parse_ki2_str(&src);
    let took = started.elapsed();
    assert!(read.is_ok(), "{:?}", read.err());
    assert!(took < std::time::Duration::from_secs(1), "{took:?}");
}

// D10: KIF and KI2 read the same content, so they are tolerant of the same
// things — a closing remark after the record, a `#` line between moves.
//
// The tolerance stops where moves start. KIF puts every move at the head of
// a numbered line, so skipping a line that does not start with a digit
// throws nothing away. KI2 writes moves anywhere along a line, so a line
// holding `▲` or `△` is never skipped: swallowing it would drop those moves
// without a word, which is the silent loss D1 exists to remove.
#[test]
fn ki2_skips_the_same_lines_kif_does_and_no_more() {
    use crate::parser::parse_ki2_str;
    for (src, moves) in [
        ("手合割：平手\n▲７六歩 △３四歩\n感想：良い将棋\n", 2),
        (
            "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断\n感想：良い\n",
            3,
        ),
        ("手合割：平手\n▲７六歩\n# メモ\n△３四歩\n", 2),
        ("手合割：平手\n▲７六歩 △３四歩\n解説A\n解説B\n", 2),
        // `変化：` with no number is a sentence opening with two
        // characters, which is what KIF makes of it too (D18).
        ("手合割：平手\n▲７六歩\n変化：ここから\n△３四歩\n", 2),
    ] {
        let jkf = parse_ki2_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
        assert_eq!(moves, jkf.moves.len() - 1, "{src:?}");
    }

    // A skipped line must not be allowed to carry a move away with it.
    for src in [
        "手合割：平手\n▲７六歩 ほげ △３四歩\n",
        "手合割：平手\n▲７六歩\nほげ △３四歩\n",
    ] {
        assert!(
            parse_ki2_str(src).is_err(),
            "{src:?} lost a move in silence"
        );
    }

    // Skipping must not swallow a `変化：` block that follows it.
    let jkf = parse_ki2_str("手合割：平手\n▲７六歩 △３四歩\n解説A\n\n変化：2手\n△８四歩\n")
        .expect("parses");
    assert!(
        jkf.moves[2].forks.is_some(),
        "the fork after the prose is gone: {:?}",
        jkf.moves
    );
}

#[test]
fn parse_empty() {
    assert_eq!(
        Ok((
            "",
            (
                JsonKifuFormat {
                    header: HashMap::new(),
                    initial: Some(Initial {
                        preset: Preset::PresetHirate,
                        data: None,
                    }),
                    moves: vec![MoveFormat::default()],
                },
                // Nothing was read, so no header line was consumed either.
                false
            )
        )),
        parse("")
    );
}

#[test]
fn parse_single_move() {
    assert_eq!(
        Ok((
            "",
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::White,
                    from: Some(ORIGIN_UNSTATED),
                    to: PlaceFormat { x: 0, y: 0 },
                    piece: Kind::FU,
                    same: Some(true),
                    promote: None,
                    capture: None,
                    relative: None,
                }),
                ..Default::default()
            }
        )),
        single_move("△同　歩")
    );
    assert_eq!(
        Ok((
            "",
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::White,
                    from: Some(ORIGIN_UNSTATED),
                    to: PlaceFormat { x: 4, y: 7 },
                    piece: Kind::GI,
                    same: None,
                    promote: Some(false),
                    capture: None,
                    relative: None,
                }),
                ..Default::default()
            }
        )),
        single_move("△４七銀不成")
    );
    assert_eq!(
        Ok((
            "",
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::White,
                    from: Some(ORIGIN_UNSTATED),
                    to: PlaceFormat { x: 9, y: 9 },
                    piece: Kind::KA,
                    same: None,
                    promote: Some(true),
                    capture: None,
                    relative: None,
                }),
                ..Default::default()
            }
        )),
        single_move("△９九角成")
    );
    assert_eq!(
        Ok((
            "",
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::Black,
                    from: Some(ORIGIN_UNSTATED),
                    to: PlaceFormat { x: 8, y: 2 },
                    piece: Kind::KI,
                    same: None,
                    promote: None,
                    capture: None,
                    relative: Some(Relative::U),
                }),
                ..Default::default()
            }
        )),
        single_move("▲８二金上")
    );
    assert_eq!(
        Ok((
            "",
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::Black,
                    from: Some(ORIGIN_UNSTATED),
                    to: PlaceFormat { x: 8, y: 2 },
                    piece: Kind::KI,
                    same: None,
                    promote: None,
                    capture: None,
                    relative: Some(Relative::M),
                }),
                ..Default::default()
            }
        )),
        single_move("▲８二金寄")
    );
    assert_eq!(
        Ok((
            "",
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::Black,
                    from: None,
                    to: PlaceFormat { x: 8, y: 2 },
                    piece: Kind::KI,
                    same: None,
                    promote: None,
                    capture: None,
                    relative: Some(Relative::H),
                }),
                ..Default::default()
            }
        )),
        single_move("▲８二金打")
    );
}

#[test]
fn parse_moves() {
    assert_eq!(
        Ok((
            "",
            vec![MoveFormat {
                comments: Some(vec![String::from("comment")]),
                ..Default::default()
            }]
        )),
        moves(
            Color::Black,
            WhereABoardCouldBe::past_a_board_for_tests(),
            "*comment\n"
        )
    );
    assert_eq!(
        Ok((
            "",
            vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 6, y: 8 },
                        piece: Kind::GI,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                },
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 3, y: 4 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                },
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 5, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            ]
        )),
        moves(
            Color::Black,
            WhereABoardCouldBe::past_a_board_for_tests(),
            "▲６八銀 △３四歩 ▲５六歩"
        )
    )
}

#[test]
fn parse_moves_with_comments() {
    assert_eq!(
        Ok((
            "",
            vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 7, y: 4 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    comments: Some(vec![
                        String::from("-2732"),
                        String::from("△３二銀(31)▲７六歩(77)△８四歩(83)"),
                    ]),
                    ..Default::default()
                },
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 7, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    comments: Some(vec![
                        String::from("2733"),
                        String::from("▲７六歩(77)△６二銀(71)▲２六歩(27)"),
                    ]),
                    ..Default::default()
                },
            ]
        )),
        moves(
            Color::Black,
            WhereABoardCouldBe::past_a_board_for_tests(),
            &r#"
△７四歩
*-2732
*△３二銀(31)▲７六歩(77)△８四歩(83)
▲７六歩
*2733
*▲７六歩(77)△６二銀(71)▲２六歩(27)
"#[1..]
        )
    );
}

/// GAP-034: a tsume record ends with 詰 rather than 詰み, with or without the
/// で — both are in Kifu-for-JS's test files and in tsshogi's.
#[test]
fn a_tsume_record_ends_with_詰_with_or_without_で() {
    for (line, want) in [
        ("まで2手で詰", MoveSpecial::SpecialTsumi),
        ("まで2手詰", MoveSpecial::SpecialTsumi),
        ("まで２手詰", MoveSpecial::SpecialTsumi),
        ("まで2手で詰み", MoveSpecial::SpecialTsumi),
        ("まで2手で不詰", MoveSpecial::SpecialFuzumi),
    ] {
        let jkf = crate::parser::parse_ki2_str(&format!("▲２六歩 △３四歩\n{line}\n"))
            .unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(
            Some(want),
            jkf.moves.last().and_then(|mf| mf.special),
            "{line}"
        );
    }
}

/// GAP-037: the square in Arabic digits, half-width (`▲26歩`, as tsshogi's
/// tests write it) or full-width in both places (`▲２６歩`, the 日本将棋連盟
/// style). A prose line that opens with a number is still prose.
#[test]
fn a_square_in_arabic_digits_is_a_square() {
    let want = crate::parser::parse_ki2_str("▲２六歩 △３四歩\n▲７六歩\n").expect("kanji ranks");
    for src in [
        "▲26歩 △34歩\n▲76歩\n",
        "▲２６歩 △３４歩\n▲７６歩\n",
        "▲26歩 ▽34歩\n▲７六歩\n",
    ] {
        let got = crate::parser::parse_ki2_str(src).unwrap_or_else(|e| panic!("{src}: {e}"));
        assert_eq!(want.moves, got.moves, "{src}");
    }
    let kif = crate::parser::parse_kif_str("1 76歩(77)\n2 34歩(33)\n").expect("KIF too");
    assert_eq!(
        Some(PlaceFormat { x: 7, y: 6 }),
        kif.moves[1].move_.map(|m| m.to)
    );
    let noted = crate::parser::parse_ki2_str("▲２六歩 △３四歩\n12月の対局\n▲７六歩\n")
        .expect("a line of prose that opens with a number is not a move");
    assert_eq!(4, noted.moves.len());
}

/// GAP-024 / GAP-050: `▼▽` and `☗☖` are the same marks as `▲△`. A `▽` in the
/// middle of a line was not refused but dropped, one move short.
#[test]
fn every_spelling_of_the_side_marks_keeps_every_move() {
    for (black, white) in [('▲', '△'), ('▼', '▽'), ('☗', '☖')] {
        let src = format!("{black}２六歩 {white}３四歩\n{black}７六歩\n");
        let jkf = crate::parser::parse_ki2_str(&src).unwrap_or_else(|e| panic!("{src}: {e}"));
        let colors: Vec<_> = jkf
            .moves
            .iter()
            .filter_map(|mf| mf.move_.map(|m| m.color))
            .collect();
        assert_eq!(
            vec![Color::Black, Color::White, Color::Black],
            colors,
            "{src}"
        );
    }
}

/// GAP-043: `<side>の反則負け` names the loser. After two moves it is Black's
/// turn, so Black losing is ILLEGAL_MOVE and White losing is a foul by White.
#[test]
fn the_side_a_foul_names_is_the_side_that_loses() {
    for (phrase, want) in [
        ("先手の反則負け", MoveSpecial::SpecialIllegalMove),
        ("後手の反則負け", MoveSpecial::SpecialIllegalActionWhite),
        ("反則負け", MoveSpecial::SpecialIllegalMove),
    ] {
        let jkf = crate::parser::parse_ki2_str(&format!("▲２六歩 △３四歩\nまで2手で{phrase}\n"))
            .unwrap_or_else(|e| panic!("{phrase}: {e}"));
        assert_eq!(
            Some(want),
            jkf.moves.last().and_then(|mf| mf.special),
            "{phrase}"
        );
    }
    // And the writer's own phrase for each reads back as itself.
    for special in [
        MoveSpecial::SpecialIllegalMove,
        MoveSpecial::SpecialIllegalActionWhite,
        MoveSpecial::SpecialIllegalActionBlack,
    ] {
        let mut jkf = crate::parser::parse_ki2_str("▲２六歩 △３四歩\n").expect("two moves");
        jkf.moves.push(MoveFormat {
            special: Some(special),
            ..Default::default()
        });
        let back =
            crate::parser::parse_ki2_str(&jkf.try_to_string(crate::Format::Ki2).expect("writes"))
                .expect("reads back");
        assert_eq!(Some(special), back.moves.last().and_then(|mf| mf.special));
    }
}

/// GAP-029: a line that opens with a note marker is a note, even when it
/// quotes a move. KIF skipped it; KI2 refused the whole file.
#[test]
fn a_note_that_quotes_a_move_is_a_note() {
    for note in ["※▲２六歩が本筋", "（△３四歩なら互角）", "【▲７六歩の変化】"]
    {
        let jkf = crate::parser::parse_ki2_str(&format!("▲２六歩 △３四歩\n{note}\n▲７六歩\n"))
            .unwrap_or_else(|e| panic!("{note}: {e}"));
        assert_eq!(4, jkf.moves.len(), "{note}");
    }
    // A mark at the head of a line still opens a move, and a broken one is
    // still refused.
    assert!(crate::parser::parse_ki2_str("▲２六歩 △３四歩\n▲有利でした\n").is_err());
}
