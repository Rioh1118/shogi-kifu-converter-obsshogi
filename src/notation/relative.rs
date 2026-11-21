//! R-NOT-004: which of several pieces made a move, in both directions.
//!
//! Reading, the traditional notation gives a destination, a piece and a
//! [`Relative`]; [`resolve_from`] finds the origin. Writing, the origin is
//! known and [`infer_relative_from_position`] finds the [`Suffix`] that names
//! it. Both ask the same question of the same candidate set, so they live
//! together: a change to one direction that the other does not follow reads
//! back as a different move.

use crate::error::NormalizeErrorKind;
use crate::jkf::*;
use shogi_core::PartialPosition;
use shogi_legality_lite::prelegality::is_valid;

/// The origin of `mmf`, worked out from the position and its `relative`.
pub(crate) fn resolve_from(
    mmf: &MoveMoveFormat,
    pos: &PartialPosition,
    to: shogi_core::Square,
) -> Result<Option<PlaceFormat>, NormalizeErrorKind> {
    let color = pos.side_to_move();
    // The candidate set the writer scans (`infer_relative_from_position`), with
    // one difference the reader cannot close: the writer adds the square the
    // record says the move came from, and the reader is looking for that square.
    // An illegal move is valid input (R-RULE-002) and `is_valid` leaves it out,
    // so a suffix written for such a move names a piece this scan does not see —
    // `to_ki2` then writes a line `parse_ki2_str` reports as ambiguous
    // (`research/90-gaps.md` GAP-020 の隣、実データでは0件).
    //
    // `LiteLegalityChecker::normal_to_candidates` is not the set either: it
    // tests full legality, so a pinned piece drops out, while the writer follows
    // `shogi_official_kifu` and counts it (R-RULE-001, D2).
    let bb = candidates_reaching(
        pos,
        to,
        shogi_core::Piece::new(shogi_core::PieceKind::from(mmf.piece), color),
    );
    match bb.count() {
        0 => Ok(None),
        1 => Ok(bb.into_iter().next().map(|sq| PlaceFormat {
            x: sq.file(),
            y: sq.rank(),
        })),
        2.. => {
            let all: Vec<_> = bb.into_iter().collect();
            let relative = mmf
                .relative
                .ok_or_else(|| NormalizeErrorKind::AmbiguousMoveFrom(all.clone()))?;
            // Ask which candidate the writer would have spelled this way. Any
            // other reading of the suffix is a second copy of R-NOT-004, and the
            // two copies drift: fixing one side and not the other makes a
            // `▲５八金直` this crate wrote come back ambiguous.
            let froms: Vec<_> = all
                .iter()
                .copied()
                .filter(|&from| suffix_for(pos, from, to, bb) == Suffix::Only(relative))
                .collect();
            if froms.len() == 1 {
                Ok(Some(PlaceFormat {
                    x: froms[0].file(),
                    y: froms[0].rank(),
                }))
            } else {
                Err(NormalizeErrorKind::AmbiguousMoveFrom(froms))
            }
        }
    }
}

/// The squares a piece of the same kind could have moved to `to` from.
///
/// This is the candidate set R-NOT-004 talks about: everything that could have
/// made the move, which is what decides whether a suffix is needed and which
/// one. `shogi_official_kifu` builds the same set by enumerating every move on
/// the board (`all_valid_moves`, 14,256 legality tests per move) and filtering;
/// asking the board directly costs one scan of the 81 squares.
fn candidates_reaching(
    pos: &PartialPosition,
    to: shogi_core::Square,
    piece: shogi_core::Piece,
) -> shogi_core::Bitboard {
    let mut candidates = shogi_core::Bitboard::empty();
    for from in shogi_core::Square::all() {
        if pos.piece_at(from) != Some(piece) {
            continue;
        }
        if [false, true]
            .into_iter()
            .any(|promote| is_valid(pos, shogi_core::Move::Normal { from, to, promote }))
        {
            candidates |= from;
        }
    }
    candidates
}

/// R-NOT-004 stage 1: 上 / 引 / 寄, and the candidates that share it.
fn by_motion(
    pos: &PartialPosition,
    from: shogi_core::Square,
    to: shogi_core::Square,
    candidates: shogi_core::Bitboard,
) -> Option<(shogi_core::Bitboard, Motion)> {
    let side = pos.side_to_move();
    let rank_of = |sq: shogi_core::Square| sq.relative_rank(side) as i8;
    let delta = (rank_of(from) - rank_of(to)).signum();
    let mut same = shogi_core::Bitboard::empty();
    for candidate in candidates {
        if (rank_of(candidate) - rank_of(to)).signum() == delta {
            same |= candidate;
        }
    }
    if same.is_empty() {
        return None;
    }
    Some((
        same,
        match delta.cmp(&0) {
            std::cmp::Ordering::Greater => Motion::Up,
            std::cmp::Ordering::Less => Motion::Down,
            std::cmp::Ordering::Equal => Motion::Across,
        },
    ))
}

/// R-NOT-004 stage 2: 左 / 直 / 右, and the candidates that share it.
///
/// The two piece groups compare different things. 金 and 銀 and the promoted
/// pieces move one square, so their candidates sit beside the destination and
/// the file offset names them — and only they can be 直. 桂 角 馬 飛 龍 come
/// from anywhere, so the two candidates are ranked against *each other*: 馬 and
/// 龍 reach a square from its own file, and comparing the origin with the
/// destination would leave no candidate at all there.
fn by_file(
    pos: &PartialPosition,
    from: shogi_core::Square,
    to: shogi_core::Square,
    candidates: shogi_core::Bitboard,
) -> Option<(shogi_core::Bitboard, Option<Side>)> {
    use shogi_core::PieceKind::*;
    let side = pos.side_to_move();
    let kind = pos.piece_at(from)?.piece_kind();
    let sign = if side == shogi_core::Color::Black {
        1
    } else {
        -1
    };

    if matches!(kind, Knight | Bishop | Rook | ProBishop | ProRook) {
        // The traditional notation gives these two names, so it can only tell
        // two apart; a third would need a spelling that does not exist.
        if candidates.count() != 2 {
            return Some((candidates, None));
        }
        let mut rest = candidates;
        let (Some(one), Some(other)) = (rest.pop(), rest.pop()) else {
            return Some((candidates, None));
        };
        if one.file() == other.file() {
            return Some((candidates, None));
        }
        let (right, left) = if one.file() as i8 * sign < other.file() as i8 * sign {
            (one, other)
        } else {
            (other, one)
        };
        let named = if from == left {
            Side::Left
        } else if from == right {
            Side::Right
        } else {
            return Some((shogi_core::Bitboard::empty(), None));
        };
        return Some((shogi_core::Bitboard::single(from), Some(named)));
    }

    let file_offset = from.file() as i8 - to.file() as i8;
    if file_offset == 0 && from.relative_rank(side) as i8 - to.relative_rank(side) as i8 > 0 {
        return Some((shogi_core::Bitboard::single(from), Some(Side::Straight)));
    }
    let named = match (file_offset * sign).cmp(&0) {
        std::cmp::Ordering::Less => Some(Side::Right),
        std::cmp::Ordering::Greater => Some(Side::Left),
        // Same file, and not moving up: 直 does not apply and there is no other
        // word for it. Stage 1 has to settle this one.
        std::cmp::Ordering::Equal => None,
    };
    let mut same = shogi_core::Bitboard::empty();
    for candidate in candidates {
        if candidate.file() as i8 - to.file() as i8 == file_offset {
            same |= candidate;
        }
    }
    Some((same, named))
}

/// 上 / 引 / 寄.
#[derive(Clone, Copy)]
enum Motion {
    Up,
    Down,
    Across,
}

/// 左 / 直 / 右.
#[derive(Clone, Copy, PartialEq)]
enum Side {
    Left,
    Straight,
    Right,
}

/// What the traditional notation says about a move.
#[derive(Clone, Copy, PartialEq)]
pub(crate) enum Suffix {
    /// Only one piece could have made the move, so nothing is written.
    Nothing,
    /// The suffix that names which one it was.
    Only(Relative),
    /// More than one piece could have made it and no spelling separates them.
    /// R-NOT-004 stage 3 calls this an error: the notation cannot say it.
    Unspellable,
}

/// The suffix for the piece on `from`, given everything that could have moved
/// to `to`.
///
/// **Both directions go through here.** The writer asks what to spell; the
/// reader asks which candidate would have been spelled the way the file reads.
/// A second copy of the rule is how 直 came to be written by one side and
/// unreadable by the other.
///
/// R-NOT-004 takes the shortest that names it: the motion alone, else the file
/// alone, else both.
fn suffix_for(
    pos: &PartialPosition,
    from: shogi_core::Square,
    to: shogi_core::Square,
    candidates: shogi_core::Bitboard,
) -> Suffix {
    if candidates.count() < 2 {
        return Suffix::Nothing;
    }
    let (Some((by_motion, motion)), Some((by_file, side))) = (
        by_motion(pos, from, to, candidates),
        by_file(pos, from, to, candidates),
    ) else {
        return Suffix::Unspellable;
    };
    if by_motion.count() == 1 {
        return Suffix::Only(match motion {
            Motion::Up => Relative::U,
            Motion::Down => Relative::D,
            Motion::Across => Relative::M,
        });
    }
    // `side` is `None` for a move the notation has no word for — a gold-like
    // piece going straight *back*, which 直 does not cover. Stage 1 had its
    // chance above; there is nothing to fall back to.
    let Some(side) = side else {
        return Suffix::Unspellable;
    };
    if by_file.count() == 1 {
        return Suffix::Only(match side {
            Side::Left => Relative::L,
            Side::Straight => Relative::C,
            Side::Right => Relative::R,
        });
    }
    if (by_file & by_motion).count() == 1 {
        return match (side, motion) {
            (Side::Left, Motion::Up) => Suffix::Only(Relative::LU),
            (Side::Left, Motion::Down) => Suffix::Only(Relative::LD),
            (Side::Left, Motion::Across) => Suffix::Only(Relative::LM),
            (Side::Right, Motion::Up) => Suffix::Only(Relative::RU),
            (Side::Right, Motion::Down) => Suffix::Only(Relative::RD),
            (Side::Right, Motion::Across) => Suffix::Only(Relative::RM),
            // 直 settles a move on its own or not at all; there is no 直上.
            (Side::Straight, _) => Suffix::Unspellable,
        };
    }
    Suffix::Unspellable
}

/// The suffix (左/右/直/上/寄/引/打) for `mv` in `pos`.
///
/// R-NOT-004: a suffix is written only when more than one piece of that kind
/// could have made the move. R-NOT-003 does the same for a drop: 打 only when a
/// piece already on the board could have gone there.
pub(crate) fn infer_relative_from_position(pos: &PartialPosition, mv: shogi_core::Move) -> Suffix {
    match mv {
        // R-NOT-003.
        shogi_core::Move::Drop { to, piece } => {
            let on_board = shogi_core::Piece::new(piece.piece_kind(), pos.side_to_move());
            if candidates_reaching(pos, to, on_board).is_empty() {
                Suffix::Nothing
            } else {
                Suffix::Only(Relative::H)
            }
        }
        shogi_core::Move::Normal { from, to, .. } => {
            let Some(piece) = pos.piece_at(from) else {
                return Suffix::Nothing;
            };
            // The piece that made the move is a candidate by definition. The
            // scan cannot see that on its own: a kifu recording an illegal move
            // is valid input (R-RULE-002), and an illegal move is exactly what
            // `is_valid` leaves out — which would make the suffix name a
            // different piece, or make the record unspellable altogether.
            let mut candidates = candidates_reaching(pos, to, piece);
            candidates |= from;
            suffix_for(pos, from, to, candidates)
        }
    }
}
