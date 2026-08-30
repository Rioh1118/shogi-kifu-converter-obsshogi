use super::kakinoki::{write_header, write_initial, write_kansuji, write_sanyou_suji};
use crate::jkf::*;
use std::fmt::{Result, Write};

/// A type that is convertible to KI2 format.
pub trait ToKi2 {
    /// Write `self` in KI2 format.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `sink` fails, or if the record holds something KI2
    /// cannot spell — a coordinate outside the board, or more pieces in hand
    /// than a set contains. Such a record comes from outside this crate; it is
    /// not a value any parser here produces.
    fn to_ki2<W: Write>(&self, sink: &mut W) -> Result;

    /// Returns `self`'s string representation, or the error [`Self::to_ki2`]
    /// gave.
    fn try_to_ki2_owned(&self) -> std::result::Result<String, std::fmt::Error> {
        let mut s = String::new();
        self.to_ki2(&mut s)?;
        Ok(s)
    }

    /// Returns `self`'s string representation.
    ///
    /// A record that cannot be spelled in KI2 yields whatever was written
    /// before the failure. Use [`Self::try_to_ki2_owned`] to see the error
    /// instead of a truncated file.
    #[deprecated(
        note = "returns a truncated string on failure, which the caller writes to disk as if it were the whole record. Use try_to_ki2_owned."
    )]
    fn to_ki2_owned(&self) -> String {
        let mut s = String::new();
        let _ = self.to_ki2(&mut s);
        s
    }
}

fn write_move_kind<W: Write>(kind: Kind, sink: &mut W) -> Result {
    match kind {
        Kind::FU => sink.write_str("歩"),
        Kind::KY => sink.write_str("香"),
        Kind::KE => sink.write_str("桂"),
        Kind::GI => sink.write_str("銀"),
        Kind::KI => sink.write_str("金"),
        Kind::KA => sink.write_str("角"),
        Kind::HI => sink.write_str("飛"),
        Kind::OU => sink.write_str("玉"),
        Kind::TO => sink.write_str("と"),
        Kind::NY => sink.write_str("成香"),
        Kind::NK => sink.write_str("成桂"),
        Kind::NG => sink.write_str("成銀"),
        Kind::UM => sink.write_str("馬"),
        Kind::RY => sink.write_str("竜"),
    }
}

/// Writes the KI2 notation for `moves`, deriving the disambiguating suffix from
/// `position` rather than from [`MoveMoveFormat::relative`].
///
/// KI2 carries no move origin, so a move that needs `左`/`右`/… and does not get
/// it cannot be read back. `relative` is derived data — a value the position
/// already determines — and trusting the field means any caller that skipped
/// [`JsonKifuFormat::populate_relative`] silently writes an unreadable file.
///
/// `position` becomes `None` once a move cannot be applied. Kifu recording an
/// illegal move are valid input (R-RULE-002), so the remaining moves fall back
/// to whatever `relative` holds instead of refusing to write.
fn write_moves<W: Write>(
    moves: &[MoveFormat],
    position: Option<shogi_core::PartialPosition>,
    sink: &mut W,
) -> Result {
    let Some((head, rest)) = moves.split_first() else {
        return Ok(());
    };
    if let Some(comments) = &head.comments {
        for comment in comments {
            if !comment.starts_with('&') {
                sink.write_char('*')?;
            }
            sink.write_str(comment)?;
            sink.write_char('\n')?;
        }
    }
    // The main line, then one `変化：N手` block per branch.
    //
    // The order is load-bearing. A reader has only the ply number to go on, so
    // a block is understood against the most recent block that starts earlier —
    // which means a branch leaving ply 6 has to be written *before* one leaving
    // ply 5, or the ply-6 block reads as a continuation of the ply-5 one.
    // Taking blocks off the end gives that, and pushing each node's branches in
    // reverse keeps siblings in their original order.
    let start = position
        .as_ref()
        .map_or(Color::Black, |pos| pos.side_to_move().into());
    let mut stack = Vec::new();
    write_line(rest, 1, position, start, &mut stack, sink)?;
    while let Some((start_ply, branch, at)) = stack.pop() {
        sink.write_fmt(format_args!("\n変化：{start_ply}手\n"))?;
        write_line(branch, start_ply, at, start, &mut stack, sink)?;
    }
    Ok(())
}

/// Writes one run of moves — the main line or one branch — and queues any
/// branches that depart from it.
///
/// Each queued branch carries the position it departs from. Deriving it instead
/// by replaying the main line is wrong for a branch inside a branch: that
/// branch leaves a line the main line never visits, so the replay lands on a
/// different board and the suffixes are spelled against the wrong candidates.
fn write_line<'a, W: Write>(
    moves: &'a [MoveFormat],
    first_ply: usize,
    mut position: Option<shogi_core::PartialPosition>,
    start: Color,
    stack: &mut Vec<(usize, &'a [MoveFormat], Option<shogi_core::PartialPosition>)>,
    sink: &mut W,
) -> Result {
    // Whether the cursor sits at the beginning of a line. Separators are
    // written *before* what they separate, because what a move needs in front
    // of it depends on what came before: a space after another move, nothing
    // after a line that is already terminated. Writing them afterwards instead
    // means an outcome line has no way to close itself, and the moves that
    // follow get swallowed as part of the `まで…` text.
    let mut at_line_start = true;
    // The ply is the number the *reader* will give this node, not the position
    // in the array. A node carrying only comments writes nothing a reader can
    // count, so counting it here would put `変化：N手` one past its move.
    let mut ply = first_ply;
    for mf in moves {
        // A branch is the alternative *to* this move (R-JKF-004), so it is
        // spelled against the position before this move is played.
        let departs_from = if mf.forks.is_some() {
            position.clone()
        } else {
            None
        };
        if let Some(mv) = &mf.move_ {
            if !at_line_start {
                sink.write_char(' ')?;
            }
            match mv.color {
                Color::Black => sink.write_char('▲')?,
                Color::White => sink.write_char('△')?,
            }
            if mv.same.is_some() {
                sink.write_str("同")?;
            } else {
                write_sanyou_suji(mv.to.x, sink)?;
                write_kansuji(mv.to.y, sink)?;
            }
            write_move_kind(mv.piece, sink)?;
            let core_move = shogi_core::Move::try_from(mv).ok();
            let relative = match (&position, core_move) {
                (Some(pos), Some(core_move)) => {
                    crate::normalizer::infer_relative_from_position(pos, core_move)
                }
                _ => mv.relative,
            };
            if let Some(relative) = relative {
                match relative {
                    Relative::L => sink.write_str("左")?,
                    Relative::C => sink.write_str("直")?,
                    Relative::R => sink.write_str("右")?,
                    Relative::U => sink.write_str("上")?,
                    Relative::M => sink.write_str("寄")?,
                    Relative::D => sink.write_str("引")?,
                    Relative::LU => sink.write_str("左上")?,
                    Relative::LM => sink.write_str("左寄")?,
                    Relative::LD => sink.write_str("左引")?,
                    Relative::RU => sink.write_str("右上")?,
                    Relative::RM => sink.write_str("右寄")?,
                    Relative::RD => sink.write_str("右引")?,
                    Relative::H => sink.write_str("打")?,
                }
            }
            if let Some(promote) = mv.promote {
                if promote {
                    sink.write_str("成")?;
                } else {
                    sink.write_str("不成")?;
                }
            }
            position = position.and_then(|mut pos| {
                let core_move = core_move?;
                pos.make_move(core_move)?;
                Some(pos)
            });
            at_line_start = false;
        } else if let Some(special) = &mf.special {
            // KI2 records the outcome as a `まで<N>手で…` line rather than as
            // another move. Dropping it makes a finished game look abandoned,
            // and KI2 has no ply numbers, so nothing downstream can tell.
            if !at_line_start {
                sink.write_char('\n')?;
            }
            // The board knows whose turn it is; the ply parity does not, since
            // a handicap starts with White (R-HC-001). Falling back to the
            // parity only matters once an illegal move has cost us the board.
            let side_to_move = position.as_ref().map_or_else(
                || crate::handicap::side_to_move_at_ply(start, ply),
                |pos| pos.side_to_move().into(),
            );
            sink.write_fmt(format_args!(
                "まで{}手で{}\n",
                ply - 1,
                special.ki2_phrase(side_to_move)
            ))?;
            at_line_start = true;
        }
        if let Some(comments) = &mf.comments {
            if !at_line_start {
                sink.write_char('\n')?;
            }
            for comment in comments {
                if !comment.starts_with('&') {
                    sink.write_char('*')?;
                }
                sink.write_str(comment)?;
                sink.write_char('\n')?;
            }
            at_line_start = true;
        }
        if let Some(forks) = &mf.forks {
            for fork in forks.iter().rev() {
                stack.push((ply, fork.as_slice(), departs_from.clone()));
            }
        }
        if mf.move_.is_some() || mf.special.is_some() {
            ply += 1;
        }
    }
    if !at_line_start {
        sink.write_char('\n')?;
    }
    Ok(())
}

impl ToKi2 for JsonKifuFormat {
    fn to_ki2<W: Write>(&self, sink: &mut W) -> Result {
        write_header(&self.header, sink)?;
        write_initial(&self.initial, true, sink)?;
        write_moves(&self.moves, self.starting_position(), sink)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
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

    /// Spells the move tree as `<ply>:<destination>` with branches in brackets,
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

    // The disambiguating suffix comes from the position, not from `relative`,
    // so a value that never went through `populate_relative` still produces KI2
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

    // KI2 records the outcome as a `まで<N>手で…` line. Writing the moves and
    // dropping that line makes a finished game look abandoned, and KI2 carries
    // no ply numbers, so nothing downstream can tell that something went
    // missing. The spellings and the `まで<N>手で` wrapper are D5's.
    #[test]
    fn outcome_survives_a_ki2_round_trip() {
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

    // Every branch has to reach the file: saving a study as `.ki2` with only
    // the main line loses the rest with no error. The block order matters as
    // much as the blocks — a reader has only the ply number to work from.
    #[test]
    fn branches_survive_a_ki2_round_trip() {
        // Two branches leaving ply 2 and one leaving ply 3, so both the sibling
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
            "every branch gets a block: {ki2:?}"
        );
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(shape(&jkf.moves[1..], 1), shape(&back.moves[1..], 1));
    }

    // A game can be interrupted and resumed, so `中断` shows up in the middle of
    // a move list. The reader takes the whole line after `まで` as the outcome
    // phrase, so a run of moves continuing on that line disappears into it.
    #[test]
    fn an_outcome_in_the_middle_does_not_swallow_the_moves_after_it() {
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
    fn a_handicap_names_the_right_side_at_the_outcome() {
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

    // A branch inside a branch leaves a line the main line never visits, so the
    // position it departs from cannot be recovered by replaying the main line.
    // Spelling it against the main line drops the suffix the reader needs
    // (R-NOT-004), and the branch comes back ambiguous and is thrown away.
    //
    // Here the golds differ between the two lines: the main line has moved 6九
    // to 7八, which cannot reach 5八, so a main-line replay sees one candidate
    // and writes no suffix at all.
    #[test]
    fn a_branch_inside_a_branch_is_spelled_from_its_own_position() {
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
            "both golds reach 5八 in the branch, so both need a suffix: {ki2:?}"
        );
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(shape(&jkf.moves[1..], 1), shape(&back.moves[1..], 1));
    }

    // A record with no header, no starting position and no moves writes
    // nothing at all: a move run that never opened a line has none to end.
    #[test]
    fn to_ki2_default() {
        assert_eq!(
            "",
            JsonKifuFormat::default()
                .try_to_ki2_owned()
                .expect("writes KI2")
        );
        assert!(crate::parser::parse_ki2_str("").is_ok());
    }

    #[test]
    fn to_ki2_moves() {
        assert_eq!(
            "▲２六歩 △８四歩 ▲２五歩\n",
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
}
