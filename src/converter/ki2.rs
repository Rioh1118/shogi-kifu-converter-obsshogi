use super::kakinoki::{
    write_comment, write_header, write_initial, write_kansuji, write_sanyou_suji,
};
use super::WriteResult;
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
    fn to_ki2<W: Write>(&self, sink: &mut W) -> WriteResult;

    /// Returns `self`'s string representation, or the error [`Self::to_ki2`]
    /// gave.
    fn try_to_ki2_owned(&self) -> std::result::Result<String, ConvertError> {
        let mut s = String::new();
        self.to_ki2(&mut s)?;
        Ok(s)
    }
}

fn write_move_kind<W: Write>(kind: Kind, sink: &mut W) -> WriteResult {
    sink.write_str(crate::notation::move_word(kind))?;
    Ok(())
}

/// Whether this move has a `不成` to write (R-NOT-005,
/// [`is_promotion_spellable`](crate::notation::is_promotion_spellable)).
///
/// The rule is asked here, and not left to the normalizer, because a record
/// reaches a writer without having been through it: past a special move the first
/// move the board cannot explain ends the tracking (R-RULE-002), a fork that
/// could not be normalized is kept as it was, and a JKF the consumer built never
/// went through it at all (R-REQ-002). What the record says a move did is
/// `promote` (D12); whether the notation has a word for it is what the piece and
/// the squares say.
fn can_promote(mv: &MoveMoveFormat) -> bool {
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
        (Ok(from), Ok(to)) => crate::notation::is_promotion_spellable(piece, from, to, color),
        // The record stated an origin and the destination is the one that went
        // missing: a `同` nothing resolved, because past a special move the position
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
) -> WriteResult {
    let Some((head, rest)) = moves.split_first() else {
        return Ok(());
    };
    if let Some(comments) = &head.comments {
        for comment in comments {
            write_comment(comment, sink)?;
        }
    }
    // The main line, then one `変化：N手` block per fork.
    //
    // The order is load-bearing. A reader has only the ply number to go on, so
    // a block is understood against the most recent block that starts earlier —
    // which means a fork leaving ply 6 has to be written *before* one leaving
    // ply 5, or the ply-6 block reads as a continuation of the ply-5 one.
    // Taking blocks off the end gives that, and pushing each node's forks in
    // reverse keeps siblings in their original order.
    let mut stack = Vec::new();
    write_line(rest, 1, position, start, &mut stack, sink)?;
    while let Some((start_ply, fork, at)) = stack.pop() {
        sink.write_fmt(format_args!("\n変化：{start_ply}手\n"))?;
        write_line(fork, start_ply, at, start, &mut stack, sink)?;
    }
    Ok(())
}

/// Writes one run of moves — the main line or one fork — and queues any
/// forks that depart from it.
///
/// Each queued fork carries the position it departs from. Deriving it instead
/// by replaying the main line is wrong for a fork inside a fork: that
/// fork leaves a line the main line never visits, so the replay lands on a
/// different board and the suffixes are spelled against the wrong candidates.
fn write_line<'a, W: Write>(
    moves: &'a [MoveFormat],
    first_ply: usize,
    mut position: Option<shogi_core::PartialPosition>,
    start: Color,
    stack: &mut Vec<(usize, &'a [MoveFormat], Option<shogi_core::PartialPosition>)>,
    sink: &mut W,
) -> WriteResult {
    // Whether the cursor sits at the beginning of a line. Separators are
    // written *before* what they separate, because what a move needs in front
    // of it depends on what came before: a space after another move, nothing
    // after a line that is already terminated. Writing them afterwards instead
    // means a special move line has no way to close itself, and the moves that
    // follow get swallowed as part of the `まで…` text.
    let mut at_line_start = true;
    // The ply a KI2 line names, not the position in the array. A node carrying
    // only comments writes nothing to number, so counting it would put
    // `変化：N手` one move past where it belongs and `まで<N>手` one too high.
    // `MoveFormat::occupies_ply` is the shared rule; the KIF writer and the
    // KI2 reader use the same one.
    let mut ply = first_ply;
    for mf in moves {
        // A fork is the alternative *to* this move (R-JKF-004), so it is
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
                    use crate::notation::relative::Suffix;
                    match crate::notation::relative::infer_relative_from_position(pos, core_move) {
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
                // No position to ask — either none was tracked, or this move
                // cannot be made into one (`同` with the square not filled in
                // yet) — so the record itself is all there is —
                // and what it says about a drop is the absence of `from`
                // (R-JKF-003). `relative` is worked out from a board, so where
                // the two disagree the record wins (D23).
                _ if mv.from.is_none() => Some(Relative::H),
                _ => mv.relative,
            };
            if let Some(relative) = relative {
                // The spelling belongs to `notation`, where the reader takes it
                // from as well: two tables of the same 13 words only disagree
                // when someone reads back a file this crate wrote.
                sink.write_str(crate::notation::relative_word(relative))?;
            }
            match mv.promote {
                // D12: a promotion is what the record says, and it is spelled
                // whatever the board makes of it.
                Some(true) => sink.write_str("成")?,
                Some(false) if can_promote(mv) => sink.write_str("不成")?,
                _ => {}
            }
            position = position.and_then(|mut pos| {
                let core_move = core_move?;
                pos.make_move(core_move)?;
                Some(pos)
            });
            at_line_start = false;
        } else if let Some(special) = &mf.special {
            // KI2 records the special move as a `まで<N>手で…` line rather than as
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
        if mf.occupies_ply() {
            ply += 1;
        }
    }
    if !at_line_start {
        sink.write_char('\n')?;
    }
    Ok(())
}

impl ToKi2 for JsonKifuFormat {
    fn to_ki2<W: Write>(&self, sink: &mut W) -> WriteResult {
        write_header(&self.header, sink)?;
        write_initial(&self.header, &self.initial, sink)?;
        // R-HC-001: only the even game starts with Black. The board says so too
        // when there is one, but a record this crate cannot turn into a position
        // still has to name the right side at its special move.
        let start = crate::handicap::starting_side(self.initial.as_ref());
        write_moves(&self.moves, self.initial_position(), start, sink)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests;
