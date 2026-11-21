use super::*;

// R-KIF-007: KIF's 反則勝ち says the move *before* it was the foul, so the
// word alone does not name the offender — whose turn it is does. The upper
// hand moves first in every handicap (R-HC-001), so reading the side off the
// ply parity records the wrong player as the one who fouled.
//
// This is the only thing the parser's own side-to-move decides: every move's
// colour is overwritten later from the position, so nothing else here fails
// when it is wrong.
#[test]
fn a_handicap_names_the_right_side_at_a_foul_win() {
    use crate::converter::ToCsa;
    for (handicap, first_move, want) in [
        ("平手", "７六歩(77)", MoveSpecial::SpecialIllegalActionBlack),
        (
            "香落ち",
            "３四歩(33)",
            MoveSpecial::SpecialIllegalActionWhite,
        ),
        (
            "四枚落ち",
            "３四歩(33)",
            MoveSpecial::SpecialIllegalActionWhite,
        ),
    ] {
        let kif = format!(
                "手合割：{handicap}\n手数----指手---------消費時間--\n   1 {first_move}\n   2 反則勝ち\n"
            );
        let jkf = crate::parser::parse_kif_str(&kif).unwrap_or_else(|e| panic!("{handicap}: {e}"));
        assert_eq!(
            Some(want),
            jkf.moves.last().and_then(|mf| mf.special),
            "reading {handicap}"
        );
        // R-CSA-007 spells the offender out, so the direction is visible.
        let csa = jkf.try_to_csa_owned().expect("writes CSA");
        let sign = if want == MoveSpecial::SpecialIllegalActionBlack {
            "+"
        } else {
            "-"
        };
        assert!(
            csa.lines().any(|l| l == format!("%{sign}ILLEGAL_ACTION")),
            "{handicap} wrote {csa:?}"
        );
    }
}

// KIF numbers a special move line as a ply of its own, so a record that was
// interrupted and resumed has more plies than moves and the number stops
// matching the turn. 反則勝ち names its winner only through whose turn it is
// (R-KIF-007), so reading the turn off the parity records the wrong player
// as the one who fouled — and `to_csa` then writes the opposite `%±`.
#[test]
fn a_special_in_the_middle_does_not_shift_the_turn() {
    // ７六歩 is Black's, so 3三→3四 at ply 3 is White's despite the odd ply.
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ３四歩(33)
   4 反則勝ち
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    assert_eq!(
        Color::White,
        jkf.moves[3].move_.expect("a move").color,
        "the move after 中断 is White's"
    );
    // White moved last, so Black is to move and 反則勝ち is Black's win —
    // which is a foul *by* White, `%-ILLEGAL_ACTION` (R-CSA-007).
    assert_eq!(
        Some(MoveSpecial::SpecialIllegalActionWhite),
        jkf.moves.last().and_then(|mf| mf.special),
    );
}

// R-KIF-007. Every word KIF defines for a special move, and what it means here.
//
// 不戦勝 / 不戦敗 have no counterpart among JKF's fourteen, so this crate
// extends the enum for them (D1). Reading them as nothing is worse: they
// are spec vocabulary, and a reader that does not know them refuses a
// valid KIF as a whole.
//
// Both are defined against Black (先手) outright, not against
// whoever is to move — unlike 反則勝ち in the same table — so the ply does
// not change what they mean.
#[test]
fn kif_special_words() {
    // Ply 2, so it is White to move and 反則勝ち accuses Black.
    for (word, want) in [
        ("投了", Some(MoveSpecial::SpecialToryo)),
        ("中断", Some(MoveSpecial::SpecialChudan)),
        ("千日手", Some(MoveSpecial::SpecialSennichite)),
        ("切れ負け", Some(MoveSpecial::SpecialTimeUp)),
        ("反則負け", Some(MoveSpecial::SpecialIllegalMove)),
        ("反則勝ち", Some(MoveSpecial::SpecialIllegalActionBlack)),
        ("持将棋", Some(MoveSpecial::SpecialJishogi)),
        ("入玉勝ち", Some(MoveSpecial::SpecialKachi)),
        ("詰み", Some(MoveSpecial::SpecialTsumi)),
        ("不詰", Some(MoveSpecial::SpecialFuzumi)),
        ("不戦勝", Some(MoveSpecial::SpecialFusensho)),
        ("不戦敗", Some(MoveSpecial::SpecialFusenpai)),
    ] {
        let line = format!("   2 {word}\n");
        let got = move_line(Color::Black, None, &line)
            .ok()
            .and_then(|(_, (_, mf))| mf.special);
        assert_eq!(want, got, "reading {word}");
    }
}

// The other direction of the same table. 反則勝ち loses which side fouled;
// the reader recovers it from the ply number.
#[test]
fn kif_special_words_are_written_back() {
    for special in MoveSpecial::ALL {
        let Some(word) = special.kif_word() else {
            // 待った and エラー have no KIF word at all.
            assert!(
                matches!(
                    special,
                    MoveSpecial::SpecialMatta | MoveSpecial::SpecialError
                ),
                "{special:?} has no KIF word"
            );
            continue;
        };
        let line = format!("   2 {word}\n");
        let got = move_line(Color::Black, None, &line)
            .ok()
            .and_then(|(_, (_, mf))| mf.special);
        assert!(
            got.is_some(),
            "{special:?} writes {word}, which cannot be read"
        );
    }
}

#[test]
fn what_the_reader_takes_and_what_the_skip_counts_are_the_same_set() {
    // The two questions — "is this one of my lines" and "what does it say" —
    // have one answer (D21). Counted-but-unreadable is allowed in one place
    // only (`   2 パス` reaches the leftover-input check instead of the
    // skip, D8); *read but not counted* is the silence D1 exists to remove,
    // and so is a reader that gets part way in and then says the line was
    // never one of its lines.
    const PLIES: [&str; 4] = ["1", "  1", "   12", "99999999999999999999"];
    const SEPARATORS: [&str; 3] = ["", " ", "\u{3000}"];
    const BODIES: [&str; 26] = [
        "投了",
        "中断",
        "詰み",
        "反則勝ち",
        "７六歩(77)",
        "同　銀(68)",
        "１三角打",
        "２二角成(88)",
        "２二角不成(88)",
        "▲７六歩(77)",
        "△３四歩(33)",
        "５五馬(66)",
        // Move lines a writer got wrong. Each says what it is and then
        // fails, so each has to be reported rather than skipped.
        "７六歩(999)",
        "７六歩(256)",
        "７六歩(1234)",
        "７六歩(00)",
        "７六歩(8)",
        "７六歩(88",
        "７六歩(ab)",
        "７六歩()",
        "７六歩（88）",
        "７六歩(八八)",
        "７六歩[88]",
        "７六歩 (77)",
        "７六歩",
        // Prose: the digits are not followed by a move at all.
        "図以下、先手優勢",
    ];
    const TIMES: [&str; 4] = ["", " ( 0:03/00:00:03)", " (00:01/00:00:01)", " ( 0:03)"];
    const TAILS: [&str; 6] = ["", "+", " ▲有利", "（まで先手良し）", "※注記", "もあった"];

    let (mut read, mut reported, mut prose) = (0, 0, 0);
    for ply in PLIES {
        for separator in SEPARATORS {
            for body in BODIES {
                for time in TIMES {
                    for tail in TAILS {
                        let line = format!("{ply}{separator}{body}{time}{tail}\n");
                        let counted = opens_numbered_line(&line);
                        match move_line(Color::Black, None, &line) {
                            Ok(_) => {
                                read += 1;
                                assert!(counted, "{line:?} is read but not counted");
                            }
                            Err(nom::Err::Failure(_)) => {
                                reported += 1;
                                assert!(counted, "{line:?} is reported but not counted");
                            }
                            // The other direction, which is what let a
                            // reader that stopped half way call the line
                            // prose: what the reader declines outright, the
                            // skip is free to take, and nothing else.
                            Err(_) => {
                                prose += 1;
                                assert!(!counted, "{line:?} is counted but declined");
                            }
                        }
                        // And the same line as the last one in a file. A
                        // record that stops inside a move line is cut short,
                        // not a shorter game (D1).
                        let cut = line.trim_end_matches('\n');
                        assert!(
                            opens_numbered_line(cut) || prose_opens(cut),
                            "{cut:?} at the end of a file is neither counted nor prose"
                        );
                    }
                }
            }
        }
    }
    // The loop is worth nothing if every line landed in one bucket.
    assert!(
        read > 500 && reported > 500 && prose > 50,
        "{read} / {reported} / {prose}"
    );
}

/// Whether the line was prose to begin with — digits and then something that
/// is not a move, which stays prose however the file ends.
fn prose_opens(line: &str) -> bool {
    !opens_numbered_line(&format!("{line}\n"))
}

#[test]
fn a_record_that_stops_inside_a_move_line_says_so() {
    // A file cut short is the one thing a reader must not hand back as a
    // shorter game: nothing downstream can tell the two apart (D1). Every
    // prefix of a move line, taken as the end of the file, has to be either
    // read or reported — never skipped.
    const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
    for line in [
        "   2 ８四歩(83)",
        "   2 投了 ( 0:03/00:00:03)",
        "2８四歩(83)",
        "   2 ８四歩(83) ( 0:03/00:00:03)",
    ] {
        for cut in 1..=line.len() {
            if !line.is_char_boundary(cut) {
                continue;
            }
            let piece = &line[..cut];
            // Up to the first digit there is nothing to have started.
            if !piece
                .trim_start_matches(is_padding)
                .starts_with(|c: char| c.is_ascii_digit())
            {
                continue;
            }
            let record = format!("{HEAD}{piece}");
            let plies = match crate::parser::parse_kif_str(&record) {
                Ok(jkf) => jkf.moves.len() - 1,
                Err(_) => continue,
            };
            assert_eq!(
                2, plies,
                "{piece:?} at the end of the file came back as a shorter game"
            );
        }
    }
}

// The shared reader is where the spelling lives now
// (`kakinoki::numbered_line`); what this side owns is the `MoveFormat` built
// out of it, including R-KIF-006's reading of a missing `成`.
#[test]
fn a_move_line_becomes_the_move_it_names() {
    for (line, expected) in [
        (
            "   1 ７六歩(77)\n",
            MoveMoveFormat {
                color: Color::Black,
                from: Some(PlaceFormat { x: 7, y: 7 }),
                to: PlaceFormat { x: 7, y: 6 },
                piece: Kind::FU,
                same: None,
                promote: Some(false),
                capture: None,
                relative: None,
            },
        ),
        (
            "   1 ４二角成(31)\n",
            MoveMoveFormat {
                color: Color::Black,
                from: Some(PlaceFormat { x: 3, y: 1 }),
                to: PlaceFormat { x: 4, y: 2 },
                piece: Kind::KA,
                same: None,
                promote: Some(true),
                capture: None,
                relative: None,
            },
        ),
        // R-KIF-006 tells a writer not to spell 不成; a reader that refuses
        // it drops what other software wrote.
        (
            "   1 ２二角不成(88)\n",
            MoveMoveFormat {
                color: Color::Black,
                from: Some(PlaceFormat { x: 8, y: 8 }),
                to: PlaceFormat { x: 2, y: 2 },
                piece: Kind::KA,
                same: None,
                promote: Some(false),
                capture: None,
                relative: None,
            },
        ),
        // A drop says nothing about promotion either way (R-NOT-005).
        (
            "   1 １三角打\n",
            MoveMoveFormat {
                color: Color::Black,
                from: None,
                to: PlaceFormat { x: 1, y: 3 },
                piece: Kind::KA,
                same: None,
                promote: None,
                capture: None,
                relative: None,
            },
        ),
    ] {
        let (_, (_, mf)) =
            move_line(Color::Black, None, line).unwrap_or_else(|e| panic!("{line:?}: {e}"));
        assert_eq!(Some(expected), mf.move_, "{line:?}");
    }
}

#[test]
fn parse_move_line() {
    assert!(move_line(Color::Black, None, "").is_err());
    assert_eq!(
        Ok((
            "",
            (
                1,
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 7, y: 7 }),
                        to: PlaceFormat { x: 7, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    comments: None,
                    time: Some(Time {
                        now: TimeFormat {
                            h: None,
                            m: 0,
                            s: 16
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 16
                        }
                    }),
                    special: None,
                    forks: None,
                }
            )
        )),
        move_line(Color::Black, None, "1 ７六歩(77) ( 0:16/00:00:16)\n")
    );
    assert_eq!(
        Ok((
            "",
            (
                3,
                MoveFormat {
                    move_: None,
                    comments: None,
                    time: Some(Time {
                        now: TimeFormat {
                            h: None,
                            m: 0,
                            s: 3
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 19
                        }
                    }),
                    special: Some(MoveSpecial::SpecialChudan),
                    forks: None,
                }
            )
        )),
        move_line(Color::Black, None, "3 中断 ( 0:03/ 0:00:19)\n")
    );
    assert_eq!(
        Ok((
            "",
            (
                1,
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 6, y: 9 }),
                        to: PlaceFormat { x: 7, y: 8 },
                        piece: Kind::KI,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    time: Some(Time {
                        now: TimeFormat {
                            h: None,
                            m: 0,
                            s: 1
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 1
                        }
                    }),
                    ..Default::default()
                }
            )
        )),
        move_line(
            Color::Black,
            None,
            "   1 ７八金(69)    (00:01 / 00:00:01)\n"
        )
    )
}

#[test]
fn parse_main_moves() {
    assert_eq!(
        Ok((
            "",
            vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 7, y: 7 }),
                        to: PlaceFormat { x: 7, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    comments: None,
                    time: Some(Time {
                        now: TimeFormat {
                            h: None,
                            m: 0,
                            s: 16
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 16
                        }
                    }),
                    special: None,
                    forks: None,
                },
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(PlaceFormat { x: 3, y: 3 }),
                        to: PlaceFormat { x: 3, y: 4 },
                        piece: Kind::FU,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    comments: None,
                    time: Some(Time {
                        now: TimeFormat {
                            h: None,
                            m: 0,
                            s: 0
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 0
                        }
                    }),
                    special: None,
                    forks: None,
                },
                MoveFormat {
                    move_: None,
                    comments: None,
                    time: Some(Time {
                        now: TimeFormat {
                            h: None,
                            m: 0,
                            s: 3
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 19
                        }
                    }),
                    special: Some(MoveSpecial::SpecialChudan),
                    forks: None,
                },
            ]
        )),
        main_moves(
            Color::Black,
            WhereABoardCouldBe::past_a_board_for_tests(),
            &r#"
1 ７六歩(77) ( 0:16/00:00:16)
2 ３四歩(33) ( 0:00/00:00:00)
3 中断 ( 0:03/ 0:00:19)
"#[1..],
        )
    );
    assert_eq!(
        Ok((
            "",
            vec![
                MoveFormat {
                    comments: Some(vec![String::from("開始局面のコメント")]),
                    ..Default::default()
                },
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 2, y: 7 }),
                        to: PlaceFormat { x: 2, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    time: Some(Time {
                        now: TimeFormat {
                            s: 1,
                            ..Default::default()
                        },
                        total: TimeFormat {
                            h: Some(0),
                            m: 0,
                            s: 1
                        }
                    }),
                    ..Default::default()
                },
            ]
        )),
        main_moves(
            Color::Black,
            WhereABoardCouldBe::past_a_board_for_tests(),
            &r#"
*開始局面のコメント
  1 ２六歩(27) ( 0:01/00:00:01)
"#[1..]
        )
    )
}

#[test]
fn parse_entire_moves() {
    let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)
   2 ８四歩(83)    (00:00 / 00:00:00)
   3 ６八銀(79)    (00:00 / 00:00:00)
   4 ３二金(41)    (00:00 / 00:00:00)
   5 ２六歩(27)    (00:00 / 00:00:00)
   6 ８五歩(84)    (00:00 / 00:00:00)
   7 ７七角(88)    (00:00 / 00:00:00)
   8 ３四歩(33)    (00:00 / 00:00:00)
   9 ７八金(69)    (00:00 / 00:00:00)
  10 ７七角成(22)  (00:00 / 00:00:00)
  11 同　銀(68)    (00:00 / 00:00:00)
  12 ２二銀(31)    (00:00 / 00:00:00)

変化：10手
  10 ３三角(22)    (00:00 / 00:00:00)
  11 ６九玉(59)    (00:00 / 00:00:00)
  12 ４二銀(31)    (00:00 / 00:00:00)
  13 ３六歩(37)    (00:00 / 00:00:00)
  14 ７七角成(33)  (00:00 / 00:00:00)

変化：5手
   5 ７七角(88)    (00:00 / 00:00:00)
   6 ３四歩(33)    (00:00 / 00:00:00)
   7 ４八銀(39)    (00:00 / 00:00:00)
   8 ６二銀(71)    (00:00 / 00:00:00)
   9 ３六歩(37)    (00:00 / 00:00:00)
  10 ８五歩(84)    (00:00 / 00:00:00)
  11 ７八金(69)    (00:00 / 00:00:00)
  12 ７四歩(73)    (00:00 / 00:00:00)

変化：9手
   9 １六歩(17)    (00:00 / 00:00:00)
  10 １四歩(13)    (00:00 / 00:00:00)
  11 ２六歩(27)    (00:00 / 00:00:00)
  12 ４二銀(31)    (00:00 / 00:00:00)
  13 ２二角成(77)  (00:00 / 00:00:00)
  14 同　金(32)    (00:00 / 00:00:00)
  15 ７七銀(68)    (00:00 / 00:00:00)
"#[1..];
    let ret = entire_moves(
        Color::Black,
        WhereABoardCouldBe::past_a_board_for_tests(),
        input,
    );
    let (rest, moves) = ret.expect("failed to parse");
    assert!(rest.is_empty());
    assert_eq!(13, moves.len());
    for (i, m) in moves.iter().enumerate() {
        match i {
            5 => {
                let forks = m.forks.as_ref().expect("no forks");
                assert_eq!(1, forks.len());
                assert_eq!(8, forks[0].len());
                assert!(forks[0].iter().any(|m| m.forks.is_none()))
            }
            10 => {
                let forks = m.forks.as_ref().expect("no forks");
                assert_eq!(1, forks.len());
                assert_eq!(5, forks[0].len());
                assert!(forks[0].iter().all(|m| m.forks.is_none()))
            }
            _ => assert!(m.forks.is_none()),
        }
    }
}

// A run numbered past the end of the run it belongs to is that run carrying
// on, not an alternative to a move that is not there (D3 rule 4). Here
// 変化:5 is three plies past 変化:2, which holds one move.
//
// Dropping it returns `Ok` with the moves gone, which is the silent loss D1
// exists to remove — and tsshogi appends. Indexing straight into the run
// panics with `index out of bounds`, which is why the lookup is
// `checked_sub`: neither the panic nor the silent drop is the answer.
#[test]
fn a_fork_numbered_past_the_end_carries_the_run_on() {
    let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)
   2 ８四歩(83)    (00:00 / 00:00:00)

変化：2
   2 ８六歩(83)    (00:00 / 00:00:00)

変化：5
   5 投了 ( 0:00/ 0:00:00)
"#[1..];
    let (_, moves) = entire_moves(
        Color::Black,
        WhereABoardCouldBe::past_a_board_for_tests(),
        input,
    )
    .expect("parses");
    let forks = moves[2]
        .forks
        .as_ref()
        .expect("変化:2 should attach at index 2");
    assert_eq!(1, forks.len());
    assert_eq!(
        2,
        forks[0].len(),
        "変化:5 continues 変化:2 rather than vanishing"
    );
    assert_eq!(
        Some(MoveSpecial::SpecialToryo),
        forks[0][1].special,
        "and it is the special move it was"
    );
}

// The same rule on the main line. A record whose ply numbers jump — which
// is what a reader that stopped on a line it could not place produces —
// must not lose the moves after the jump.
#[test]
fn a_fork_numbered_past_the_main_line_carries_it_on() {
    let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)

変化：9
   9 ３四歩(33)    (00:00 / 00:00:00)
"#[1..];
    let (_, moves) = entire_moves(
        Color::Black,
        WhereABoardCouldBe::past_a_board_for_tests(),
        input,
    )
    .expect("parses");
    assert_eq!(3, moves.len(), "index 0 plus two moves");
    assert!(
        moves.iter().all(|m| m.forks.is_none()),
        "nothing forked: {moves:?}"
    );
}

// A `変化：` block can open with a node carrying only comments: JKF allows
// one (R-JKF-002), KIF has no numbered line for it, and `to_kif` writes it
// straight after the `変化：N手` header. The reader has to take back what
// the writer puts out — refusing it made this crate reject its own output,
// and the KI2 reader produces exactly this shape (`parser::ki2::tests::
// a_comment_only_node_does_not_shift_the_outcome_ply`).
//
// A `&` bookmark in the same place was worse than an error: it was taken
// for an unreadable line and skipped, so it came back missing with nothing
// said (R-KIF-011).
//
// The ply is the number on the line, not the position in the array, so the
// comment node must not shift where the block attaches either.
#[test]
fn a_fork_can_open_with_a_comment_and_survives_a_kif_round_trip() {
    use crate::converter::ToKif;
    use crate::parser::{parse_ki2_str, parse_kif_str};
    for opening in ["*このあとは", "&しおり"] {
        let ki2 = format!(
                "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断\n\n変化：2手\n{opening}\n△８四歩\nまで2手で反則勝ち\n"
            );
        let jkf = parse_ki2_str(&ki2).unwrap_or_else(|e| panic!("{opening}: {e}"));
        let fork = &jkf.moves[2]
            .forks
            .as_ref()
            .unwrap_or_else(|| panic!("{opening}: no fork"))[0];
        assert_eq!(3, fork.len(), "{opening}: comment, move, special move");

        let kif = jkf
            .try_to_kif_owned()
            .unwrap_or_else(|e| panic!("{opening}: {e}"));
        let back = parse_kif_str(&kif).unwrap_or_else(|e| {
            panic!("{opening}: this crate cannot read its own KIF: {e}\n{kif}")
        });
        assert_eq!(jkf, back, "{opening}: {kif:?}");
    }
}

// The same node must not move the fork either. Here the block that opens
// with a comment is itself the target of a later `変化：`, so counting array
// slots instead of ply numbers hangs the second block off the wrong move.
#[test]
fn a_comment_node_does_not_shift_where_a_fork_attaches() {
    use crate::parser::parse_kif_str;
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２六歩(27)

変化：2
*このあとは
   2 ８四歩(83)
   3 ２六歩(27)

変化：3
   3 ７八金(69)
";
    let jkf = parse_kif_str(kif).expect("parses");
    let outer = &jkf.moves[2].forks.as_ref().expect("a fork at ply 2")[0];
    assert_eq!(
        [false, true, true],
        [0, 1, 2].map(|i| outer[i].occupies_ply()),
        "comment node first, then plies 2 and 3: {outer:?}"
    );
    // `変化：3` is an alternative to ply 3, which is `outer[2]` — not
    // `outer[3]`, which does not exist.
    assert!(
        outer[2].forks.is_some(),
        "the inner fork hangs off ply 3: {outer:?}"
    );
}

// R-KIF-002: a blank line and a `#` line may sit anywhere in the move list.
// Ending the run of moves on one leaves every move after it to be read as
// a fork of a ply that is not there, and dropped — the record is `Ok`
// with one move (GAP-008). tsshogi reads all three.
//
// The blank line is the harder of the two: a skip that starts on the newline
// itself takes the line after it as this line's content, so the move
// following a blank line goes with it (`not_move_line` declines line ends
// for that reason).
#[test]
fn a_blank_or_program_comment_line_does_not_end_the_move_list() {
    use crate::parser::parse_kif_str;
    const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
    const TAIL: &str = "   2 ３四歩(33)\n   3 ２六歩(27)\n";
    for middle in ["", "\n", "# メモ\n", "\n# メモ\n\n", "   \n"] {
        let jkf = parse_kif_str(&format!("{HEAD}{middle}{TAIL}"))
            .unwrap_or_else(|e| panic!("{middle:?} was rejected: {e}"));
        assert_eq!(3, jkf.moves.len() - 1, "{middle:?}");
    }
}

// The blank line before a `変化：` block is what ends the run, so tolerating
// blank lines inside a run must not swallow the block that follows one. The
// two readings of a blank line compete: the run skips it and carries on,
// and the fork reader needs it to have ended the run. What settles it is
// what comes after — a numbered move line continues the run, a `変化：`
// header does not.
#[test]
fn a_blank_line_still_lets_a_fork_start() {
    use crate::parser::parse_kif_str;
    const HEAD: &str =
        "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ３四歩(33)\n";
    const FORK: &str = "\n変化：2\n   2 ８四歩(83)\n";

    for (middle, main_line) in [("", 2), ("\n   3 ２六歩(27)\n", 3)] {
        let jkf = parse_kif_str(&format!("{HEAD}{middle}{FORK}"))
            .unwrap_or_else(|e| panic!("{middle:?} was rejected: {e}"));
        assert_eq!(main_line, jkf.moves.len() - 1, "{middle:?}");
        let forks = jkf.moves[2]
            .forks
            .as_ref()
            .unwrap_or_else(|| panic!("no fork at ply 2 for {middle:?}"));
        assert_eq!(1, forks.len(), "{middle:?}");
        assert_eq!(1, forks[0].len(), "the fork holds its one move");
    }
}

// A text file need not end with a newline, and kifu written by hand or by
// other software turn up without one. Requiring one dropped whatever was on
// the last line — a move, a comment, or `まで<N>手で…` — and returned `Ok`,
// so the record came back one line short with nothing said about it
// (R-REQ-004).
#[test]
fn the_last_line_is_read_without_a_trailing_newline() {
    use crate::parser::parse_kif_str;
    const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";

    let moves = parse_kif_str(&format!("{HEAD}   2 ３四歩(33)")).expect("parses");
    assert_eq!(
        2,
        moves.moves.len() - 1,
        "the second move is on the last line"
    );

    let comment = parse_kif_str(&format!("{HEAD}*memo")).expect("parses");
    assert_eq!(
        Some(&vec![String::from("memo")]),
        comment.moves[1].comments.as_ref(),
        "the comment is on the last line"
    );

    let special = parse_kif_str(&format!("{HEAD}   2 投了")).expect("parses");
    assert_eq!(
        Some(MoveSpecial::SpecialToryo),
        special.moves[2].special,
        "the special move is on the last line"
    );
}

/// GAP-035: 封じ手 ends the first day of a two-day game. Not in R-KIF-007,
/// but the 名人戦 record in Kifu-for-JS's test files has it as a numbered
/// line with a time, the same shape as any other special move.
#[test]
fn a_sealed_move_is_read_and_written_back() {
    let src = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)   ( 0:01/00:00:01)\n   2 封じ手       (25:00/04:07:00)\n";
    let jkf = crate::parser::parse_kif_str(src).expect("a sealed move reads");
    assert_eq!(Some(MoveSpecial::SpecialFuujite), jkf.moves[2].special);
    let back =
        crate::parser::parse_kif_str(&jkf.try_to_string(crate::Format::Kif).expect("writes"))
            .expect("reads back");
    assert_eq!(Some(MoveSpecial::SpecialFuujite), back.moves[2].special);
    let ki2 = crate::parser::parse_ki2_str("▲７六歩\nまで1手で封じ手\n").expect("KI2 reads");
    assert_eq!(Some(MoveSpecial::SpecialFuujite), ki2.moves[2].special);
}

/// GAP-038: the padding that lines `同` up with a two-character square can
/// come after the piece instead of after `同`.
#[test]
fn padding_after_the_piece_is_padding() {
    let head = "1 ７六歩(77)\n2 ３四歩(33)\n3 ２二角成(88)\n";
    let want =
        crate::parser::parse_kif_str(&format!("{head}4 同　銀(31)\n")).expect("the usual spelling");
    for line in ["4 同銀　(31)", "4 同銀(31)", "4 同　銀　(31)"] {
        let got = crate::parser::parse_kif_str(&format!("{head}{line}\n"))
            .unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(want.moves, got.moves, "{line}");
    }
}

/// GAP-039: a diagram written as its nine ranks alone, without the file
/// numbers and the frame (tsshogi's `position.spec.ts`), is the same position
/// as the framed one.
#[test]
fn a_board_without_its_frame_is_the_same_board() {
    let ranks = "| ・ ・ ・ ・v玉 ・ ・ ・ ・|一\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n| ・ ・ ・v香 角 ・ ・ ・ ・|三\n| ・ ・ ・ ・v歩 ・ ・ ・ ・|四\n| ・ ・ ・ 銀 ・ ・ ・ ・ ・|五\n| ・ ・ ・ 玉 ・ ・ ・v龍 ・|六\n| ・ ・ 歩 ・ 金 歩 ・ 歩 ・|七\n| ・ ・ ・ ・ ・ ・ 桂 ・ ・|八\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n";
    let white = "後手の持駒：歩八　香　桂二　銀二　金二　角　\n";
    let black = "先手の持駒：歩六　香二　桂　銀　金　飛　\n先手番\n";
    let frame = "+---------------------------+\n";
    let framed = format!("{white}  ９ ８ ７ ６ ５ ４ ３ ２ １\n{frame}{ranks}{frame}{black}");
    let want = crate::parser::parse_kif_str(&framed).expect("the framed diagram");
    let bare = crate::parser::parse_kif_str(&format!("{white}{ranks}{black}"))
        .expect("the same ranks without the frame");
    assert_eq!(want.initial, bare.initial);
    assert!(want.initial.is_some_and(|i| i.data.is_some()));
}

/// GAP-021: a note after the handicap's name is not part of it. Taken for an
/// unknown name, the record fell back to 平手 and every side was reversed.
#[test]
fn a_note_after_the_handicap_does_not_hide_it() {
    let moves = "手数----指手---------消費時間--\n   1 ３四歩(33)\n   2 ７六歩(77)\n";
    let want = crate::parser::parse_kif_str(&format!("手合割：香落ち\n{moves}")).expect("plain");
    for line in [
        "手合割：香落ち（30分）",
        "手合割：香落ち ※持ち時間各30分",
        "手合割：香落ち【練習】",
    ] {
        let got = crate::parser::parse_kif_str(&format!("{line}\n{moves}"))
            .unwrap_or_else(|e| panic!("{line}: {e}"));
        assert_eq!(want.initial, got.initial, "{line}");
        assert_eq!(want.moves, got.moves, "{line}");
    }
    // A different word after the name is not a note.
    let other = crate::parser::parse_kif_str("手合割：香落ち戦\n").expect("kept as text");
    assert_eq!(Some(&"香落ち戦".to_owned()), other.header.get("手合割"));
}
