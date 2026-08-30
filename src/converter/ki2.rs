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
    let mut stack = Vec::new();
    write_line(rest, 1, position, &mut stack, sink)?;
    while let Some((start_ply, branch, at)) = stack.pop() {
        sink.write_fmt(format_args!("\n変化：{start_ply}手\n"))?;
        write_line(branch, start_ply, at, &mut stack, sink)?;
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
    stack: &mut Vec<(usize, &'a [MoveFormat], Option<shogi_core::PartialPosition>)>,
    sink: &mut W,
) -> Result {
    // Tracks whether the next write starts a line, so the end-of-game line does
    // not get appended to the run of moves.
    let mut at_line_start = true;
    let mut it = moves.iter().enumerate().peekable();
    while let Some((index, mf)) = it.next() {
        let ply = first_ply + index;
        // A branch is the alternative *to* this move (R-JKF-004), so it is
        // spelled against the position before this move is played.
        let departs_from = if mf.forks.is_some() {
            position.clone()
        } else {
            None
        };
        if let Some(mv) = &mf.move_ {
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
            // another move. Dropping it is how a saved game came back looking
            // like it had been abandoned.
            if !at_line_start {
                sink.write_char('\n')?;
            }
            let side_to_move = [Color::White, Color::Black][ply % 2];
            sink.write_fmt(format_args!(
                "まで{}手で{}",
                ply - 1,
                special.ki2_phrase(side_to_move)
            ))?;
            at_line_start = false;
        }
        if let Some(comments) = &mf.comments {
            sink.write_char('\n')?;
            for comment in comments {
                if !comment.starts_with('&') {
                    sink.write_char('*')?;
                }
                sink.write_str(comment)?;
                sink.write_char('\n')?;
            }
            at_line_start = true;
        } else if it.peek().is_some_and(|(_, next)| next.move_.is_some()) {
            // Only between moves. The outcome starts its own line, so a
            // separator here would be trailing whitespace.
            sink.write_char(' ')?;
        }
        if let Some(forks) = &mf.forks {
            for fork in forks.iter().rev() {
                stack.push((ply, fork.as_slice(), departs_from.clone()));
            }
        }
    }
    sink.write_char('\n')?;
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
            jkf.to_ki2_owned().contains("▲５三角左成"),
            "expected a disambiguated move, got {:?}",
            jkf.to_ki2_owned()
        );
        // The field being filled in must not change the answer.
        jkf.populate_relative().expect("populates");
        assert!(jkf.to_ki2_owned().contains("▲５三角左成"));
    }

    // KI2 records the outcome as a `まで<N>手で…` line. Writing the moves and
    // dropping that line makes a finished game look abandoned, and KI2 carries
    // no ply numbers, so nothing downstream can tell that something went
    // missing. The spellings follow tsshogi (R-KI2-006).
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
            let ki2 = jkf.to_ki2_owned();
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

    // Branches used to be dropped entirely: saving a study as `.ki2` kept the
    // main line and threw the rest away, with no error. The block order matters
    // as much as the blocks — a reader has only the ply number to work from.
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
        let ki2 = jkf.to_ki2_owned();
        assert_eq!(
            3,
            ki2.lines().filter(|l| l.starts_with("変化：")).count(),
            "every branch gets a block: {ki2:?}"
        );
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(shape(&jkf.moves[1..], 1), shape(&back.moves[1..], 1));
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
            let ki2 = jkf.to_ki2_owned();
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
        let ki2 = jkf.to_ki2_owned();
        // 4九 and 6八 differ in rank, so R-NOT-004 stage 1 settles it: 上 and 寄.
        // Replaying the main line instead sees only 4九 and writes neither.
        assert!(
            ki2.contains("▲５八金上") && ki2.contains("▲５八金寄"),
            "both golds reach 5八 in the branch, so both need a suffix: {ki2:?}"
        );
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(shape(&jkf.moves[1..], 1), shape(&back.moves[1..], 1));
    }

    #[test]
    fn to_ki2_default() {
        assert_eq!("\n", JsonKifuFormat::default().to_ki2_owned());
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
            .to_ki2_owned()
        );
    }
}
