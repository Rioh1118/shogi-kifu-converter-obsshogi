use super::kakinoki::{
    write_comment, write_header, write_initial, write_kansuji, write_sanyou_suji,
};
use super::WriteResult as Result;
use crate::error::ConvertError;
use crate::jkf::*;
use std::fmt::Write;

/// A type that is convertible to KI2 format.
pub trait ToKi2 {
    /// Write `self` in KI2 format.
    ///
    /// # Errors
    ///
    /// Returns [`ConvertError::Write`] if `sink` fails, and otherwise the
    /// variant naming what KI2 cannot spell: [`InvalidSquare`] for a coordinate
    /// outside the board, [`UnspellableNumber`] for more pieces in hand than a
    /// set contains, [`UnknownPreset`] for a handicap with no board. A
    /// caller that has just failed to save a game has to tell these apart —
    /// they are different things to say to the user.
    ///
    /// [`InvalidSquare`]: ConvertError::InvalidSquare
    /// [`UnspellableNumber`]: ConvertError::UnspellableNumber
    /// [`UnknownPreset`]: ConvertError::UnknownPreset
    /// [`UnspellableMove`] is KI2's own: the traditional notation has no suffix
    /// for a move two pieces could have made (R-NOT-004), and writing it bare
    /// produces a file this crate cannot read back (R-KI2-003).
    ///
    /// [`UnspellableMove`]: ConvertError::UnspellableMove
    fn to_ki2<W: Write>(&self, sink: &mut W) -> Result;

    /// Returns `self`'s string representation, or the error [`Self::to_ki2`]
    /// gave.
    fn try_to_ki2_owned(&self) -> std::result::Result<String, ConvertError> {
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
    }?;
    Ok(())
}

/// Whether this move has a `不成` to write (R-NOT-005,
/// [`promotion_is_spellable`](crate::notation::promotion_is_spellable)).
///
/// The rule is asked here, and not left to the normalizer, because a record
/// reaches a writer without having been through it: past an outcome the first
/// move the board cannot explain ends the tracking (R-RULE-002), a branch that
/// could not be normalized is kept as it was, and a JKF the consumer built never
/// went through it at all (R-REQ-002). What the record says a move did is
/// `promote` (D12); whether the notation has a word for it is what the piece and
/// the squares say.
fn promotion_was_on_the_table(mv: &MoveMoveFormat) -> bool {
    // A drop enters the board unpromoted, and R-NOT-005 has no word for one
    // whichever end of it the camp is at (R-JKF-003: no origin is what says a
    // move is a drop).
    let Some(from) = &mv.from else {
        return false;
    };
    let piece = shogi_core::PieceKind::from(mv.piece);
    let color = shogi_core::Color::from(mv.color);
    match (
        shogi_core::Square::try_from(from),
        shogi_core::Square::try_from(&mv.to),
    ) {
        (Ok(from), Ok(to)) => crate::notation::promotion_is_spellable(piece, from, to, color),
        // The record stated an origin and the destination is the one that went
        // missing: a `同` nothing resolved, because past an outcome the position
        // stops being tracked and `to` stays at (0, 0)
        // (`research/90-gaps.md` GAP-025). R-NOT-005 asks which end of the move
        // the enemy camp is at, and this move has no end to name. Nothing is
        // lost by leaving the word out — the origin the record did state is not
        // a word, so there is no wording of the record's to keep.
        (Ok(_), Err(_)) => false,
        // The origin is off the board — `normalizer::ORIGIN_UNSTATED`, the
        // record never stated one (KI2 has no origins, R-KI2-003) and the
        // position could not supply it. The rule cannot be asked either way, but
        // here the record may have written `不成` itself, and that word is all
        // there is: between dropping one it wrote and writing one a known
        // position would not have, the first loses something and the second does
        // not (D4). Both arms, because a `同` read from KI2 has neither end —
        // asking about the destination alone drops the word this reader's own
        // output put there.
        (Err(_), Ok(_)) | (Err(_), Err(_)) => piece.promote().is_some(),
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
    start: Color,
    sink: &mut W,
) -> Result {
    let Some((head, rest)) = moves.split_first() else {
        return Ok(());
    };
    if let Some(comments) = &head.comments {
        for comment in comments {
            write_comment(comment, sink)?;
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
    // The ply a KI2 line names, not the position in the array. A node carrying
    // only comments writes nothing to number, so counting it would put
    // `変化：N手` one move past where it belongs and `まで<N>手` one too high.
    // `MoveFormat::occupies_a_ply` is the shared rule; the KIF writer and the
    // KI2 reader use the same one.
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
                    use crate::normalizer::Suffix;
                    match crate::normalizer::infer_relative_from_position(pos, core_move) {
                        Suffix::Nothing => None,
                        Suffix::Only(relative) => Some(relative),
                        // R-NOT-004 stage 3: more than one piece could have made
                        // this move and the notation has no way to say which.
                        // Writing it bare produces KI2 that cannot be read back
                        // (R-KI2-003), and the record it came from is the only
                        // copy — so refuse rather than save something that will
                        // not open.
                        Suffix::Unspellable => {
                            return Err(ConvertError::UnspellableMove(core_move))
                        }
                    }
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
            match mv.promote {
                // D12: a promotion is what the record says, and it is spelled
                // whatever the board makes of it.
                Some(true) => sink.write_str("成")?,
                Some(false) if promotion_was_on_the_table(mv) => sink.write_str("不成")?,
                _ => {}
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
                write_comment(comment, sink)?;
            }
            at_line_start = true;
        }
        if let Some(forks) = &mf.forks {
            for fork in forks.iter().rev() {
                stack.push((ply, fork.as_slice(), departs_from.clone()));
            }
        }
        if mf.occupies_a_ply() {
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
        write_initial(&self.header, &self.initial, sink)?;
        // R-HC-001: only the even game starts with Black. The board says so too
        // when there is one, but a record this crate cannot turn into a position
        // still has to name the right side at its outcome.
        let start = crate::handicap::starting_side(self.initial.as_ref());
        write_moves(&self.moves, self.starting_position(), start, sink)?;
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
    // a move list. KI2 records an outcome as a `まで…` line and nothing else
    // (R-KI2-006 / D5), and the moves after it have to go on their own line: the
    // reader treats a run of moves that fills the rest of a `まで…` line as the
    // line below, whose newline is gone, and refuses the record (D17). So the
    // writer's newline there is not cosmetic — without it the file it produces
    // does not read back.
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
        // the text and where the branch hangs.
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

    // The companion to the case above, for the other half of `occupies_a_ply`:
    // an outcome does take a ply number. `中断` sits mid-list in a game that was
    // interrupted and resumed, so this is not a tail the writer can stop at —
    // every `変化：N手` after it is off by one if the node is skipped.
    #[test]
    fn a_mid_list_outcome_consumes_a_ply() {
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

    // R-NOT-005: `不成` exists only for a promotable piece with the enemy camp at
    // one end of the move. `△６八玉不成` is not something the notation can say.
    //
    // The moves after an outcome are where this shows. `中断` takes a ply without
    // taking a turn, so ply 3 is read as White's while its gold is Black's: the
    // board cannot explain that move, tracking ends there (R-RULE-002), and
    // nothing rewrites what the record said from then on. What the record said is
    // `promote: false`, because a KIF states 不成 by leaving `成` off
    // (R-KIF-006).
    #[test]
    fn no_不成_where_the_notation_has_no_word_for_it() {
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
        // is written on comes out wrong past an outcome (`research/90-gaps.md`
        // GAP-025), and which end of the move the enemy camp is at follows from
        // the side — so pinning it would pin the wrong turn as correct.

        // A drop has an origin at neither end (R-JKF-003), so there is no move for
        // R-NOT-005 to be asked about and the enemy camp under it changes nothing.
        let drop = r#"{"color":0,"to":{"x":2,"y":2},"piece":"GI","promote":false}"#;
        assert!(!written(drop).contains("不成"), "{}", written(drop));

        // An origin the record never stated is the other end of the same
        // question, and it goes the other way: the rule cannot be asked, so the
        // record's own word is kept rather than dropped.
        let unstated =
            r#"{"color":0,"from":{"x":0,"y":0},"to":{"x":5,"y":6},"piece":"FU","promote":false}"#;
        assert!(written(unstated).contains("不成"), "{}", written(unstated));

        // A destination nothing resolved is not the same question. There is no
        // square to say which end of the move the camp is at, and `同` is how
        // the move is spelled — the word would be about nowhere.
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
    // past an outcome nothing resolves the destination either (GAP-025). The
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
        let mut jkf =
            crate::parser::parse_ki2_str("手合割：平手\n▲７六歩 △３四歩\n").expect("parses");
        jkf.header.insert(
            "note".to_owned(),
            "序盤は▲７六歩 △３四歩 の出だし".to_owned(),
        );
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads its own file back");
        assert_eq!(2, back.moves.len() - 1, "{ki2}");
        assert_eq!(jkf.header, back.header, "{ki2}");
    }

    // A record with no header, no starting position and no moves has the
    // starting position as the only thing left to write. Without it the file is
    // zero bytes and the `Ok` says nothing is wrong, which no caller can tell
    // from a save that was cut short. Naming where the game starts is the least
    // a kifu can say, and it is what KIF and CSA write for the same record.
    #[test]
    fn to_ki2_names_the_starting_position_of_an_empty_record() {
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
}
