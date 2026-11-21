use super::*;

fn black_pawn_7g7f() -> MoveFormat {
    MoveFormat {
        move_: Some(MoveMoveFormat {
            color: Color::Black,
            from: Some(PlaceFormat { x: 7, y: 7 }),
            to: PlaceFormat { x: 7, y: 6 },
            piece: Kind::FU,
            same: None,
            promote: None,
            capture: None,
            relative: None,
        }),
        ..Default::default()
    }
}

fn white_pawn_3c3d() -> MoveFormat {
    MoveFormat {
        move_: Some(MoveMoveFormat {
            color: Color::White,
            from: Some(PlaceFormat { x: 3, y: 3 }),
            to: PlaceFormat { x: 3, y: 4 },
            piece: Kind::FU,
            same: None,
            promote: None,
            capture: None,
            relative: None,
        }),
        ..Default::default()
    }
}

/// Spells the move tree as `<ply>:<destination>` with forks in brackets,
/// so a round trip can be compared as a whole rather than field by field.
fn shape(moves: &[MoveFormat], first_ply: usize) -> String {
    moves
        .iter()
        .enumerate()
        .map(|(i, mf)| {
            let ply = first_ply + i;
            let head = mf
                .move_
                .map(|mv| format!("{ply}:{}{}", mv.to.x, mv.to.y))
                .unwrap_or_else(|| format!("{ply}:{:?}", mf.special));
            match &mf.forks {
                Some(forks) => {
                    let inner: Vec<_> = forks.iter().map(|f| shape(f, ply)).collect();
                    format!("{head}[{}]", inner.join("|"))
                }
                None => head,
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

// A drop is a drop however far the writer got with the board. Position
// tracking stops at the first move that cannot be played — an illegal move
// is a legitimate record (R-RULE-002) — and past that point the only thing
// that says "this was a drop" is `from: None` (R-JKF-003). Dropping 打 there
// makes the move read back as coming from `(0, 0)`, and `to_kif` then
// refuses the whole record (R-KI2-003, D4).
#[test]
fn a_drop_keeps_its_mark_after_the_board_is_lost() {
    use crate::parser::{parse_ki2_str, parse_kif_str};
    // `９九角(88)` cannot be played, so the writer has no position after it.
    let jkf = parse_kif_str(concat!(
        "手合割：平手\n手数----指手---------消費時間--\n",
        "   1 ７六歩(77)\n   2 中断\n   3 ９九角(88)\n   4 ５五歩打\n"
    ))
    .expect("an illegal move is still a record");
    assert!(jkf.moves[4].move_.as_ref().expect("a move").from.is_none());
    let ki2 = jkf.try_to_ki2_owned().expect("writes");
    assert!(ki2.contains('打'), "{ki2}");
    let read_back = parse_ki2_str(&ki2).expect("reads back");
    assert!(
        read_back.moves[4]
            .move_
            .as_ref()
            .expect("a move")
            .from
            .is_none(),
        "the drop stopped being a drop: {ki2}"
    );
}

// The disambiguating suffix comes from the position, not from `relative`, so
// a value that never went through `populate_relative` still produces KI2
// that can be read back. Two black bishops on 7a and 3a both reach 5c.
#[test]
fn disambiguation_does_not_depend_on_relative_field() {
    let src = "\
手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ 角 ・v玉 ・ 角 ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ ・ ・ ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 ５三角成(71)   ( 0:00/00:00:00)
";
    let mut jkf = crate::parser::parse_kif_str(src).expect("parses");
    assert_eq!(
        None,
        jkf.moves[1].move_.expect("a move").relative,
        "the KIF path leaves `relative` empty; the point is that KI2 still works"
    );
    assert!(
        jkf.try_to_ki2_owned()
            .expect("writes KI2")
            .contains("▲５三角左成"),
        "expected a disambiguated move, got {:?}",
        jkf.try_to_ki2_owned().expect("writes KI2")
    );
    // The field being filled in must not change the answer.
    jkf.populate_relative().expect("populates");
    assert!(jkf
        .try_to_ki2_owned()
        .expect("writes KI2")
        .contains("▲５三角左成"));
}

// KI2 records the special move as a `まで<N>手で…` line. Writing the moves and
// dropping that line makes a finished game look abandoned, and KI2 carries
// no ply numbers, so nothing downstream can tell that something went
// missing. The spellings and the `まで<N>手で` wrapper are D5's.
#[test]
fn special_survives_a_ki2_round_trip() {
    for (word, want, phrase) in [
        ("投了", MoveSpecial::SpecialToryo, "まで2手で後手の勝ち"),
        ("千日手", MoveSpecial::SpecialSennichite, "まで2手で千日手"),
        (
            "切れ負け",
            MoveSpecial::SpecialTimeUp,
            "まで2手で時間切れにより後手の勝ち",
        ),
        (
            "反則負け",
            MoveSpecial::SpecialIllegalMove,
            "まで2手で先手の反則負け",
        ),
        (
            "反則勝ち",
            MoveSpecial::SpecialIllegalActionWhite,
            "まで2手で先手の反則勝ち",
        ),
        ("持将棋", MoveSpecial::SpecialJishogi, "まで2手で持将棋"),
        (
            "入玉勝ち",
            MoveSpecial::SpecialKachi,
            "まで2手で先手の入玉勝ち",
        ),
        ("詰み", MoveSpecial::SpecialTsumi, "まで2手で詰み"),
        ("不詰", MoveSpecial::SpecialFuzumi, "まで2手で不詰"),
    ] {
        let kif = format!(
                "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ３四歩(33)\n   3 {word}\n"
            );
        let jkf = crate::parser::parse_kif_str(&kif)
            .unwrap_or_else(|e| panic!("failed to parse {word}: {e}"));
        assert_eq!(
            Some(want),
            jkf.moves.last().and_then(|mf| mf.special),
            "reading {word}"
        );
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains(phrase), "{word} wrote {ki2:?}");
        let back = crate::parser::parse_ki2_str(&ki2)
            .unwrap_or_else(|e| panic!("failed to read back {word}: {e}"));
        assert_eq!(
            Some(want),
            back.moves.last().and_then(|mf| mf.special),
            "round trip for {word}"
        );
    }
}

// Every fork has to reach the file: saving a study as `.ki2` with only
// the main line loses the rest with no error. The block order matters as
// much as the blocks — a reader has only the ply number to work from.
#[test]
fn forks_survive_a_ki2_round_trip() {
    // Two forks leaving ply 2 and one leaving ply 3, so both the sibling
    // order and the deeper-ply-first rule are exercised. The blocks are in
    // the order KIF itself uses: deepest departure first.
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２六歩(27)
   4 投了

変化：3手
   3 ７八金(69)

変化：2手
   2 ８四歩(83)
   3 ２六歩(27)

変化：2手
   2 ４四歩(43)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert_eq!(
        3,
        ki2.lines().filter(|l| l.starts_with("変化：")).count(),
        "every fork gets a block: {ki2:?}"
    );
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(shape(&jkf.moves[1..], 1), shape(&back.moves[1..], 1));
}

// A game can be interrupted and resumed, so `中断` shows up in the middle of
// a move list. KI2 records a special move as a `まで…` line and nothing else
// (R-KI2-006 / D5), and the moves after it have to go on their own line: the
// reader treats a run of moves that fills the rest of a `まで…` line as the
// line below, whose newline is gone, and refuses the record (D17). So the
// writer's newline there is not cosmetic — without it the file it produces
// does not read back.
#[test]
fn a_special_in_the_middle_does_not_swallow_the_moves_after_it() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ３四歩(33)
   4 ２六歩(27)
   5 投了
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(
        shape(&jkf.moves[1..], 1),
        shape(&back.moves[1..], 1),
        "wrote {ki2:?}"
    );
}

// The upper hand moves first in every handicap (R-HC-001), so the parity of
// the ply does not say whose turn it is. Both records below resign at ply 2,
// and the side that resigned is the opposite one — reading the side off the
// parity names the loser as the winner for every handicap record.
#[test]
fn a_handicap_names_the_right_side_at_the_special() {
    for (handicap, first_move, want) in [
        ("平手", "７六歩(77)", "まで1手で先手の勝ち"),
        ("香落ち", "３四歩(33)", "まで1手で後手の勝ち"),
        ("四枚落ち", "３四歩(33)", "まで1手で後手の勝ち"),
    ] {
        let kif = format!(
            "手合割：{handicap}\n手数----指手---------消費時間--\n   1 {first_move}\n   2 投了\n"
        );
        let jkf = crate::parser::parse_kif_str(&kif)
            .unwrap_or_else(|e| panic!("failed to parse {handicap}: {e}"));
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains(want), "{handicap} wrote {ki2:?}");
        let back = crate::parser::parse_ki2_str(&ki2)
            .unwrap_or_else(|e| panic!("failed to read back {handicap}: {e}"));
        assert_eq!(
            Some(MoveSpecial::SpecialToryo),
            back.moves.last().and_then(|mf| mf.special),
            "round trip for {handicap}"
        );
    }
}

// `%+ILLEGAL_ACTION` is a foul *by* Black, so White wins (R-CSA-007). The
// KI2 phrase names the winner, so both directions have to survive; deriving
// the winner from whose turn it is collapses them onto one spelling and the
// side that committed the foul comes back swapped.
#[test]
fn both_directions_of_a_foul_win_survive_a_ki2_round_trip() {
    for (special, phrase) in [
        (
            MoveSpecial::SpecialIllegalActionBlack,
            "まで2手で後手の反則勝ち",
        ),
        (
            MoveSpecial::SpecialIllegalActionWhite,
            "まで2手で先手の反則勝ち",
        ),
    ] {
        let jkf = JsonKifuFormat {
            initial: Some(Initial {
                preset: Preset::PresetHirate,
                data: None,
            }),
            moves: vec![
                MoveFormat::default(),
                black_pawn_7g7f(),
                white_pawn_3c3d(),
                MoveFormat {
                    special: Some(special),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains(phrase), "{special:?} wrote {ki2:?}");
        let back = crate::parser::parse_ki2_str(&ki2)
            .unwrap_or_else(|e| panic!("failed to read back {special:?}: {e}"));
        assert_eq!(
            Some(special),
            back.moves.last().and_then(|mf| mf.special),
            "round trip for {special:?}"
        );
    }
}

// The KIF writer has the same rule and `a_comment_only_node_does_not_consume
// _a_ply` covers it there. KI2 needs its own: it carries no move numbers, so
// a `まで<N>手` that counted a comment node is a number nothing downstream
// can check, and every `変化：N手` after it attaches one move too late.
#[test]
fn a_comment_only_node_does_not_consume_a_ply() {
    let jkf = JsonKifuFormat {
        initial: Some(Initial {
            preset: Preset::PresetHirate,
            data: None,
        }),
        moves: vec![
            MoveFormat::default(),
            black_pawn_7g7f(),
            MoveFormat {
                comments: Some(vec!["ここで長考".to_owned()]),
                ..Default::default()
            },
            MoveFormat {
                forks: Some(vec![vec![MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(PlaceFormat { x: 8, y: 3 }),
                        to: PlaceFormat { x: 8, y: 4 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }]]),
                ..white_pawn_3c3d()
            },
            MoveFormat {
                special: Some(MoveSpecial::SpecialToryo),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    // Two moves precede the resignation, and White made the last of them.
    assert!(ki2.contains("まで2手で後手の勝ち"), "{ki2:?}");
    assert!(ki2.contains("変化：2手"), "{ki2:?}");
    // KI2 has no node boundaries, so the comment comes back attached to the
    // move before it rather than on a node of its own. What must survive is
    // the text and where the fork hangs.
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(
        "1:76 2:34[2:84] 3:Some(SpecialToryo)",
        shape(&back.moves[1..], 1),
        "{ki2:?}"
    );
    assert!(
        back.moves
            .iter()
            .any(|mf| mf.comments.as_ref() == Some(&vec!["ここで長考".to_owned()])),
        "the comment survives: {:?}",
        back.moves
    );
}

// The companion to the case above, for the other half of `occupies_ply`:
// a special move does take a ply number. `中断` sits mid-list in a game that was
// interrupted and resumed, so this is not a tail the writer can stop at —
// every `変化：N手` after it is off by one if the node is skipped.
#[test]
fn a_mid_list_special_consumes_a_ply() {
    let jkf = crate::parser::parse_kif_str(
        "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ３四歩(33)
   4 ２六歩(27)

変化：4手
   4 ８四歩(83)
",
    )
    .expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(ki2.contains("変化：4手"), "{ki2:?}");
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(
        "1:76 2:Some(SpecialChudan) 3:34 4:26[4:84]",
        shape(&back.moves[1..], 1),
        "{ki2:?}"
    );
}

// What lands on disk, compared with the notation rules rather than with a
// round trip.
//
// A round trip cannot see any of this. `normalize` recomputes `same` and
// `promote` from the position, so dropping either from the writer still
// reads back identically — and since `41b8583` the reader and the writer
// share one suffix rule, so an error in that rule moves both sides the same
// way and the round trip stays green.
#[test]
fn ki2_spells_each_rule_the_way_the_notation_says() {
    const OTHER: &str = "手合割：その他\n後手の持駒：なし\n  ９ ８ ７ ６ ５ ４ ３ ２ １\n\
+---------------------------+\n";
    let board = |rows: [&str; 9], hands: &str, side: &str, mv: &str| {
        format!(
            "{OTHER}{}+---------------------------+\n先手の持駒：{hands}\n{side}\n\
手数----指手---------消費時間--\n{mv}",
            rows.iter()
                .zip(["一", "二", "三", "四", "五", "六", "七", "八", "九"])
                .map(|(row, rank)| format!("{row}|{rank}\n"))
                .collect::<String>()
        )
    };
    const EMPTY: &str = "| ・ ・ ・ ・ ・ ・ ・ ・ ・";

    // R-NOT-002: the same square as the move before it is written 同.
    let same = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ３四歩(33)\n   3 ２二角成(88)\n   4 同　銀(31)\n"
            .to_owned();
    // R-NOT-005: a move touching the enemy camp that declines promotion.
    let unpromoted = board(
        [
            "|v玉 ・ ・ ・ ・ ・ ・ ・ ・",
            EMPTY,
            "| ・ ・ ・ ・ ・ ・ 銀 ・ ・",
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            "| 玉 ・ ・ ・ ・ ・ ・ ・ ・",
        ],
        "なし",
        "先手番",
        "   1 ３二銀(33)\n",
    );
    // R-NOT-003: no bishop on the board can reach 4五, so no 打.
    let drop = board(
        [
            "|v玉 ・ ・ ・ ・ ・ ・ ・ ・",
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            "| 玉 ・ ・ ・ ・ ・ ・ ・ ・",
        ],
        "角",
        "先手番",
        "   1 ４五角打\n",
    );
    // R-NOT-004 from the other seat. White sits at the top of the diagram,
    // so its left is the low file (R-HC-002) — the gold on 4一 is 左 and the
    // one on 6一 is 右, the opposite of what the same squares mean to Black.
    let gote = board(
        [
            "|v玉 ・ ・v金 ・v金 ・ ・ ・",
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            EMPTY,
            "| 玉 ・ ・ ・ ・ ・ ・ ・ ・",
        ],
        "なし",
        "後手番",
        "   1 ５二金(41)\n",
    );

    for (kif, want) in [
        (&same, "▲７六歩 △３四歩 ▲２二角成 △同銀"),
        (&unpromoted, "▲３二銀不成"),
        (&drop, "▲４五角"),
        (&gote, "△５二金左"),
    ] {
        let ki2 = crate::parser::parse_kif_str(kif)
            .unwrap_or_else(|e| panic!("{kif}\n{e}"))
            .try_to_ki2_owned()
            .expect("writes KI2");
        assert!(
            ki2.lines().any(|line| line == want),
            "expected {want:?} in {ki2:?}"
        );
    }
}

// A fork inside a fork leaves a line the main line never visits, so the
// position it departs from cannot be recovered by replaying the main line.
// Spelling it against the main line drops the suffix the reader needs
// (R-NOT-004), and the fork comes back ambiguous and is thrown away.
//
// Here the golds differ between the two lines: the main line has moved 6九
// to 7八, which cannot reach 5八, so a main-line replay sees one candidate
// and writes no suffix at all.
#[test]
fn a_fork_inside_a_fork_is_spelled_from_its_own_position() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ７八金(69)
   4 ８四歩(83)

変化：2手
   2 ８四歩(83)
   3 ６八金(69)
   4 ８五歩(84)
   5 ５八金(49)

変化：5手
   5 ５八金(68)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    // 4九 and 6八 differ in rank, so R-NOT-004 stage 1 settles it: 上 and 寄.
    // Replaying the main line instead sees only 4九 and writes neither.
    assert!(
        ki2.contains("▲５八金上") && ki2.contains("▲５八金寄"),
        "both golds reach 5八 in the fork, so both need a suffix: {ki2:?}"
    );
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(shape(&jkf.moves[1..], 1), shape(&back.moves[1..], 1));
}

// R-NOT-005: `不成` exists only for a promotable piece with the enemy camp at
// one end of the move. `△６八玉不成` is not something the notation can say.
//
// The moves after a special move are where this shows. `中断` takes a ply without
// taking a turn, so ply 3 is read as White's while its gold is Black's: the
// board cannot explain that move, tracking ends there (R-RULE-002), and
// nothing rewrites what the record said from then on. What the record said is
// `promote: false`, because a KIF states 不成 by leaving `成` off
// (R-KIF-006).
#[test]
fn a_piece_with_no_promoted_form_gets_no_不成() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ５八金(69)
   4 ３四歩(33)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(
        ki2.contains("△５八金 ") || ki2.ends_with("△５八金\n"),
        "a gold has no 成 and no 不成: {ki2:?}"
    );
    // What the pawn on the line after it gets is not fixed here. The side it
    // is written on comes out wrong past a special move (`research/90-gaps.md`
    // GAP-025), and which end of the move the enemy camp is at follows from
    // the side — so pinning it would pin the wrong turn as correct.
}

// A drop has an origin at neither end (R-JKF-003), so there is no move for
// R-NOT-005 to be asked about and the enemy camp under it changes nothing.
#[test]
fn a_drop_gets_no_不成() {
    let drop = r#"{"color":0,"to":{"x":2,"y":2},"piece":"GI","promote":false}"#;
    assert!(!written(drop).contains("不成"), "{}", written(drop));
}

// An origin the record never stated (`normalizer::ORIGIN_UNSTATED`) leaves
// R-NOT-005 with no square to ask about, and then the record's own word is
// all there is: dropping one it wrote loses something, writing one a known
// position would not have does not (D4).
#[test]
fn an_origin_the_record_never_stated_keeps_the_word_it_wrote() {
    let unstated =
        r#"{"color":0,"from":{"x":0,"y":0},"to":{"x":5,"y":6},"piece":"FU","promote":false}"#;
    assert!(written(unstated).contains("不成"), "{}", written(unstated));
}

// A destination nothing resolved is not the same question. There is no
// square to say which end of the move the camp is at, `同` is how the move
// is spelled — the word would be about nowhere — and the origin the record
// did state is not a word, so there is nothing of the record's to keep.
#[test]
fn a_destination_nothing_resolved_gets_no_不成() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ２二角成(88)
   4 同　銀(31)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(ki2.contains("▲同銀"), "{ki2:?}");
    assert!(!ki2.contains("不成"), "{ki2:?}");
}

// A `同` read from KI2 has neither end: KI2 states no origin (R-KI2-003) and
// past a special move nothing resolves the destination either (GAP-025). The
// record's own `不成` is then the only thing that says what the move did
// (D4), so a writer that asks only about the destination writes a file
// shorter than the one it read — and the consumer saves over the original
// (R-REQ-002).
#[test]
fn a_同不成_read_from_ki2_keeps_the_word_the_record_wrote() {
    let ki2 = "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断\n▲９九角 △同銀不成\n";
    let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
    let written = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(written.contains("△同銀不成"), "{written:?}");
    let back = crate::parser::parse_ki2_str(&written).expect("reads back");
    assert_eq!(jkf, back, "{written:?}");
}

/// The KI2 for a record of one move, spelled as JKF.
fn written(mv: &str) -> String {
    let json = format!(
        r#"{{"header":{{}},"initial":{{"preset":"HIRATE"}},"moves":[{{}},{{"move":{mv}}}]}}"#
    );
    let jkf: JsonKifuFormat = serde_json::from_str(&json).expect("reads the JKF");
    jkf.try_to_ki2_owned().expect("writes KI2")
}

// A header value the consumer filled in can hold anything (R-KIF-004), moves
// among them. What this crate writes, this crate has to be able to read:
// refusing the file it just produced is the one thing a round trip cannot
// survive.
#[test]
fn a_header_value_that_quotes_moves_is_written_and_read_back() {
    let mut jkf = crate::parser::parse_ki2_str("手合割：平手\n▲７六歩 △３四歩\n").expect("parses");
    jkf.header.insert(
        "note".to_owned(),
        "序盤は▲７六歩 △３四歩 の出だし".to_owned(),
    );
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads its own file back");
    assert_eq!(2, back.moves.len() - 1, "{ki2}");
    assert_eq!(jkf.header, back.header, "{ki2}");
}

// A record with no header, no initial position and no moves has the
// initial position as the only thing left to write. Without it the file is
// zero bytes and the `Ok` says nothing is wrong, which no caller can tell
// from a save that was cut short. Naming where the game starts is the least
// a kifu can say, and it is what KIF and CSA write for the same record.
#[test]
fn to_ki2_names_the_initial_position_of_an_empty_record() {
    let ki2 = JsonKifuFormat::default()
        .try_to_ki2_owned()
        .expect("writes KI2");
    assert_eq!("手合割：平手\n", ki2);
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(1, back.moves.len(), "R-JKF-001: the slot, and no move");
    assert_eq!(Some(Preset::PresetHirate), back.initial.map(|i| i.preset));
}

#[test]
fn to_ki2_moves() {
    assert_eq!(
        "手合割：平手\n▲２六歩 △８四歩 ▲２五歩\n",
        JsonKifuFormat {
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 2, y: 7 }),
                        to: PlaceFormat { x: 2, y: 6 },
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
                        color: Color::White,
                        from: Some(PlaceFormat { x: 8, y: 3 }),
                        to: PlaceFormat { x: 8, y: 4 },
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
                        from: Some(PlaceFormat { x: 2, y: 6 }),
                        to: PlaceFormat { x: 2, y: 5 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            ],
            ..Default::default()
        }
        .try_to_ki2_owned()
        .expect("writes KI2")
    );
}
