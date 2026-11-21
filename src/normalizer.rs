use crate::error::{ConvertError, NormalizeError, NormalizeErrorKind};
use crate::jkf::*;
use crate::notation::kind_from_piece_kind;
use crate::notation::relative::{infer_relative_from_position, Suffix};
use shogi_core::PartialPosition;

/// The `from` of a move whose origin the notation does not carry.
///
/// JKF has no way to say "the origin is not stated": leaving `from` out means
/// the move is a *drop* (R-JKF-003). KI2 needs to say it — the traditional
/// notation gives a destination and a disambiguating suffix, never an origin
/// (R-KI2-003) — so the KI2 reader writes this square, which is off the board,
/// and [`normalize`](JsonKifuFormat::normalize) resolves it against the
/// position.
///
/// Reading a plain absent `from` as "look it up" instead turns every drop into
/// whichever board piece could have reached the square, which takes that piece
/// off the board and leaves the hand untouched.
pub(crate) const ORIGIN_UNSTATED: PlaceFormat = PlaceFormat { x: 0, y: 0 };

fn add_timeformat(lhs: &TimeFormat, rhs: &TimeFormat) -> TimeFormat {
    // Widen before adding: two times that each fit in the field need not.
    let total = (lhs.h.unwrap_or_default() as u64 + rhs.h.unwrap_or_default() as u64) * 3600
        + (lhs.m as u64 + rhs.m as u64) * 60
        + (lhs.s as u64 + rhs.s as u64);
    TimeFormat {
        // Hours past what the field holds saturate rather than wrap. A
        // cumulative time that long is broken input, and a wrapped value would
        // be a plausible-looking lie.
        h: Some((total / 3600).min(u16::MAX as u64) as u16),
        m: ((total / 60) % 60) as u16,
        s: (total % 60) as u16,
    }
}

/// Where a move's `color` comes from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ColorSource {
    /// The record states each move's side (CSA, JKF). A `color` that
    /// disagrees with the side to move is an error.
    Recorded,
    /// The reader numbered the moves and guessed the side from the ply (KIF,
    /// KI2). `color` is overwritten with the side to move in the position.
    Position,
}

/// Whether `relative` (左/右/上/...) is filled in from the position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RelativeFill {
    /// Fill every `relative` that is `None`.
    Fill,
    /// Leave `relative` as read. A KIF states the origin, so the inference is
    /// dead work on that path (R-REQ-006).
    Skip,
}

/// What normalization does with the two fields a record may leave to it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Normalization {
    pub(crate) color: ColorSource,
    pub(crate) relative: RelativeFill,
}

impl Normalization {
    /// What a KIF record needs: the side from the position, no `relative`.
    pub(crate) const KIF: Self = Self {
        color: ColorSource::Position,
        relative: RelativeFill::Skip,
    };
    /// What a KI2 record needs: the side from the position, `relative` filled.
    pub(crate) const KI2: Self = Self {
        color: ColorSource::Position,
        relative: RelativeFill::Fill,
    };
    /// What a CSA or JKF record needs: the side as recorded, `relative` filled.
    pub(crate) const RECORDED: Self = Self {
        color: ColorSource::Recorded,
        relative: RelativeFill::Fill,
    };

    fn from_flags(correct_color: bool, infer_relative: bool) -> Self {
        Self {
            color: if correct_color {
                ColorSource::Position
            } else {
                ColorSource::Recorded
            },
            relative: if infer_relative {
                RelativeFill::Fill
            } else {
                RelativeFill::Skip
            },
        }
    }
}

impl JsonKifuFormat {
    /// Normalizes the JKF data.
    ///
    /// `correct_color`: when true, each move's `color` is overwritten with the
    /// side to move in the position (KIF, KI2: the reader guessed it from the
    /// ply). When false, a `color` that disagrees is an error (CSA, JKF: the
    /// record states it).
    ///
    /// `infer_relative`: when false, `relative` (左/右/上/...) is left as read.
    /// [`Self::populate_relative`] fills it in later.
    ///
    /// # A handicap numbered from Black
    ///
    /// Before anything else, a record that starts from a named handicap and
    /// gives its first move to Black has every main-line `color` swapped. The
    /// upper hand moves first in a handicap (R-HC-001), so a reader that
    /// numbered from Black got every side backwards. This runs whatever
    /// `correct_color` says.
    ///
    /// # What the position decides, and what the record does
    ///
    /// For a move that has an origin, `piece`, `same` and `capture` are worked
    /// out from the position and replace whatever the input held. They describe
    /// the board, not the game: which piece stands on `from`, what it took,
    /// whether the square is the one the move before went to. An input that
    /// disagrees was describing a different board.
    ///
    /// A drop has no origin (R-JKF-003), so there is no square to ask about and
    /// those three are left as they came. `time.total` is recomputed for every
    /// move, drops included.
    ///
    /// `promote` is not one of them either. Only the record says whether a move
    /// promoted — no board can be asked after the fact — so a promotion the
    /// input states is kept, and the position is consulted only to decide
    /// whether a move that did *not* promote is worth recording as `false`
    /// (R-NOT-005).
    ///
    /// `from` is worked out from the position only where the record wrote an
    /// origin it could not read — `ORIGIN_UNSTATED`, the square off the board.
    /// A `from` that is simply absent is a drop (R-JKF-003) and stays absent.
    ///
    /// `relative` is filled in only when `infer_relative` is set, which
    /// `parse_kif_str` does not do (R-REQ-006): a JKF read from a KIF has none,
    /// and [`Self::populate_relative`] is what fills them.
    ///
    /// `same` is read before it is written: it is what says a move's destination
    /// is the previous move's, which is the only way `to` can be filled in.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizeError`] if a move before the record's first special move
    /// cannot be resolved against the position it is played from.
    ///
    /// Past a special move it does not: a record need not continue the position
    /// before it (R-RULE-002), so the first move the board cannot explain ends
    /// the tracking, and every field from there on is left as the input had it.
    /// **Whatever the parser accepted, this accepts.**
    #[deprecated(
        note = "two bare bools say nothing at the call site. Use normalize(), and populate_relative() when relative is wanted; the parse_* functions already normalize."
    )]
    pub fn normalize_with_options(
        &mut self,
        correct_color: bool,
        infer_relative: bool,
    ) -> Result<(), NormalizeError> {
        self.normalize_as(Normalization::from_flags(correct_color, infer_relative))
    }

    /// [`Self::normalize_with_options`], with the two choices named.
    pub(crate) fn normalize_as(&mut self, how: Normalization) -> Result<(), NormalizeError> {
        normalize_initial(self)?;
        let pos = if let Some(initial) = &self.initial {
            if !matches!(initial.preset, Preset::PresetHirate | Preset::PresetOther)
                && self
                    .moves
                    .get(1)
                    .and_then(|mf| mf.move_.map(|mmf| mmf.color == Color::Black))
                    .unwrap_or_default()
            {
                for mv in self.moves.iter_mut().skip(1) {
                    if let Some(mmf) = &mut mv.move_ {
                        mmf.color = match mmf.color {
                            Color::Black => Color::White,
                            Color::White => Color::Black,
                        };
                    }
                }
            }
            PartialPosition::try_from(initial).map_err(|err| NormalizeError::at(0, err))?
        } else {
            PartialPosition::startpos()
        };
        let (_, rest) = match self.moves.split_first_mut() {
            Some(split) => split,
            // Index 0 is the initial position's comments, so an empty `moves`
            // has no plies to normalize rather than being an error.
            None => return Ok(()),
        };
        normalize_moves(rest, 1, pos, [TimeFormat::default(); 2], how)?;
        Ok(())
    }

    /// Normalizes the JKF data, inferring `relative` for every move.
    ///
    /// Equivalent to [`Self::normalize_with_options`] with `infer_relative` set.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizeError`] if a move cannot be resolved against the
    /// position it is played from.
    #[deprecated(
        note = "a bare bool says nothing at the call site. Use normalize(); the parse_* functions already normalize."
    )]
    pub fn normalize_with_color_correction(
        &mut self,
        correct_color: bool,
    ) -> Result<(), NormalizeError> {
        self.normalize_as(Normalization::from_flags(correct_color, true))
    }

    /// Normalizes the JKF data, requiring the move colors to already be right.
    ///
    /// Equivalent to [`Self::normalize_with_options`] with `correct_color`
    /// cleared and `infer_relative` set: a color that disagrees with the
    /// position's side to move is an error rather than something to overwrite.
    /// Use this for sources where the color is recorded explicitly (CSA, JKF)
    /// and [`Self::normalize_with_color_correction`] for those where it is
    /// derived from the move number (KIF, KI2).
    ///
    /// On a move with an origin, `piece`, `same` and `capture` are recomputed
    /// from the position and overwrite whatever the input held; `promote` is the
    /// record's to state. See [`Self::normalize_with_options`].
    ///
    /// # Errors
    ///
    /// Returns [`NormalizeError`] if a move cannot be resolved against the
    /// position it is played from.
    pub fn normalize(&mut self) -> Result<(), NormalizeError> {
        self.normalize_as(Normalization::RECORDED)
    }

    /// The position the moves start from, or `None` if `initial` cannot be
    /// turned into one.
    pub(crate) fn initial_position(&self) -> Option<PartialPosition> {
        match &self.initial {
            Some(initial) => PartialPosition::try_from(initial).ok(),
            None => Some(PartialPosition::startpos()),
        }
    }

    /// Fills in `relative` (左/右/上/...) for every move whose `relative` is
    /// `None`, replaying the position from the initial state.
    ///
    /// KIF parsing skips the inference — a KIF states the origin, so the suffix
    /// is dead work on that path (R-REQ-006). Call this when `relative` itself
    /// is wanted, which is when the record is going out as JKF: `to_ki2` works
    /// the suffix out from the position and does not read the field (D2).
    ///
    /// # Errors
    ///
    /// Returns [`NormalizeError`] for a move the position before it does not
    /// explain — but only up to the first special move. Past one, a record need not
    /// continue the position before it (R-RULE-002), so the walk stops tracking
    /// the board and leaves the rest of the record alone. **Whatever the parser
    /// accepted, this accepts.**
    pub fn populate_relative(&mut self) -> Result<(), NormalizeError> {
        let pos = if let Some(initial) = &self.initial {
            PartialPosition::try_from(initial).map_err(|err| NormalizeError::at(0, err))?
        } else {
            PartialPosition::startpos()
        };
        match self.moves.split_first_mut() {
            Some((_, rest)) => populate_relative_moves(rest, 1, pos),
            None => Ok(()),
        }
    }
}

/// Folds a board that matches a named handicap back into its preset.
///
/// The comparison walks the same table the writers use, so a handicap added
/// there is folded here without a second list to keep in step.
fn normalize_initial(jkf: &mut JsonKifuFormat) -> Result<(), NormalizeError> {
    if let Some(initial) = &mut jkf.initial {
        let Some(data) = initial.data else {
            return Ok(());
        };
        for handicap in crate::handicap::HANDICAPS {
            let matches = crate::handicap::board(handicap.preset)
                .is_some_and(|board| board == data.board)
                && data.color == crate::handicap::side_to_move(handicap.preset)
                && data.hands == [Hand::empty(); 2];
            if matches {
                *initial = Initial {
                    preset: handicap.preset,
                    data: None,
                };
                break;
            }
        }
    }
    Ok(())
}

fn normalize_move(
    mmf: &mut MoveMoveFormat,
    pos: &PartialPosition,
    how: Normalization,
) -> Result<shogi_core::Move, NormalizeErrorKind> {
    if how.color == ColorSource::Position {
        // Correct the color from the position's side to move
        // (KIF/KI2 parser assigns color by move number parity, which is wrong for 後手番 games)
        match pos.side_to_move() {
            shogi_core::Color::Black => mmf.color = Color::Black,
            shogi_core::Color::White => mmf.color = Color::White,
        }
    } else if matches!(
        (mmf.color, pos.side_to_move()),
        (Color::Black, shogi_core::Color::White) | (Color::White, shogi_core::Color::Black)
    ) {
        return Err(NormalizeErrorKind::InvalidColor);
    }
    if mmf.same.is_some() {
        mmf.to = pos
            .last_move()
            .map(|mv| PlaceFormat {
                x: mv.to().file(),
                y: mv.to().rank(),
            })
            .ok_or(NormalizeErrorKind::NoLastMove)?;
    }
    let to = shogi_core::Square::try_from(&mmf.to)?;
    if mmf.from == Some(ORIGIN_UNSTATED) {
        mmf.from = crate::notation::relative::resolve_from(mmf, pos, to)?;
    }
    if let Some(pf) = &mmf.from {
        if let Ok(from) = pf.try_into() {
            // Retrieve piece
            let piece = match pos.piece_at(from) {
                Some(piece) => piece,
                None => return Err(NormalizeErrorKind::NoPieceAt(from)),
            };
            let from_piece_kind = piece.piece_kind();
            // Set same?
            mmf.same = if pos
                .last_move()
                .map(|last| to == last.to())
                .unwrap_or_default()
            {
                Some(true)
            } else {
                None
            };
            mmf.promote = decide_promote(&*mmf, from_piece_kind, from, to, pos.side_to_move());
            mmf.piece = kind_from_piece_kind(from_piece_kind);
            // Set capture?
            mmf.capture = pos
                .piece_at(to)
                .map(|p| kind_from_piece_kind(p.piece_kind()));
        } else {
            // An origin off the board is a coordinate we could not read, not a
            // statement that the piece came from the hand. JKF says a drop by
            // leaving `from` out (R-JKF-003), and turning one into the other
            // loses the square for good — the writers reject the same value
            // rather than spell `(00)`, which KIF has no meaning for
            // (R-KIF-005).
            return Err(ConvertError::InvalidSquare((pf.x, pf.y)).into());
        }
    }
    let mv = shogi_core::Move::try_from(&*mmf)?;
    // Set relative?
    if how.relative == RelativeFill::Fill && mmf.relative.is_none() {
        // A move the notation cannot spell has no suffix to record; the writer
        // is where that has to become an error, not the JKF field.
        mmf.relative = match infer_relative_from_position(pos, mv) {
            Suffix::Only(relative) => Some(relative),
            Suffix::Nothing | Suffix::Unspellable => None,
        };
    }
    Ok(mv)
}

/// Whether `mmf` promoted, as the record has it (D12).
///
/// The record is what says a move promoted, and the board cannot argue: a
/// promotion is spelled out in every notation this crate reads. KIF and KI2
/// write `成` and the reader puts that in `promote` (R-KIF-006 / R-NOT-005);
/// CSA has no `成` and names the piece the move left behind instead
/// (R-CSA-007), which arrives as a `piece` that is the promoted form of the one
/// standing on `from`.
///
/// Deciding it from the board instead drops a `成` on a move the rules do not
/// allow to promote — a recorded illegal move is valid input (R-RULE-002) —
/// without a word, leaving a record that spells the move without it. A promotion
/// the piece has no promoted form for is not written off here either:
/// `make_move` refuses it later, and the error names the ply.
///
/// The piece name is read only where the record left `promote` empty. It is
/// CSA's way of stating a promotion, not a second opinion on a record that
/// already stated one: a `false` overruled that way turns a spelling that
/// disagrees with the board — `７五と` on a square holding a pawn — into a `成`
/// the record does not have.
///
/// A JKF read in from outside is not told apart from a CSA here, so the piece
/// name is read for it too. It only says anything where `piece` is the promoted
/// form of what stands on `from`, which in a JKF means the record and the board
/// disagree — and D12 answers that with the board. The reading is inconsistent
/// there; what it costs is one more wrong field in a record that is already
/// wrong (D12).
///
/// What the position decides is the other direction — whether a move that did
/// *not* promote is one where promoting was on the table, and so worth recording
/// as `false` at all. R-NOT-005: a promotable piece with the enemy camp at one
/// end of the move.
fn decide_promote(
    mmf: &MoveMoveFormat,
    from_piece_kind: shogi_core::PieceKind,
    from: shogi_core::Square,
    to: shogi_core::Square,
    side: shogi_core::Color,
) -> Option<bool> {
    let promoted = mmf.promote.unwrap_or_else(|| {
        from_piece_kind.promote() == Some(shogi_core::PieceKind::from(mmf.piece))
    });
    if promoted {
        Some(true)
    } else if crate::notation::is_promotion_spellable(from_piece_kind, from, to, side) {
        Some(false)
    } else {
        None
    }
}

/// Whether a walk over a run of moves still knows the board.
///
/// A record carries on past a special move — a game that was interrupted and
/// resumed picks up from wherever it left off — so a move that will not apply
/// to the position before it is not a broken record (R-RULE-002). It is the
/// point where the walk stops knowing the board, and everything after it has to
/// be left alone rather than rewritten against a position it never came from.
/// Before any special move, the same failure *is* a broken record.
///
/// The rule is here rather than inside a walk because every walk over the tree
/// needs it, and one that grows its own copy grows the same hole:
/// `populate_relative` had no notion of it and returned `Err` for records the
/// parser and `normalize` both accept (GAP-016).
struct Board {
    after_special: bool,
    known: bool,
}

impl Board {
    const fn new() -> Self {
        Self {
            after_special: false,
            known: true,
        }
    }

    /// Whether the position is still being tracked.
    const fn is_known(&self) -> bool {
        self.known
    }

    /// Notes a node that holds no move. A special move among them is what makes a
    /// later unplayable move something other than a broken record.
    fn saw(&mut self, mf: &MoveFormat) {
        self.after_special |= mf.special.is_some();
    }

    /// Reports a move the position before it does not explain.
    ///
    /// Past a special move this drops the board and returns `Ok`; before one it is
    /// the error `err` builds.
    fn unplayable(&mut self, err: impl FnOnce() -> NormalizeError) -> Result<(), NormalizeError> {
        if self.after_special {
            self.known = false;
            Ok(())
        } else {
            Err(err())
        }
    }
}

/// Normalizes a run of moves, `first_ply` being the ply `moves[0]` sits at.
///
/// The ply is carried so that an error can say which move it is about: the
/// caller has a file on disk and needs to be pointed at a line in it. The main
/// line starts at 1 because index 0 of `moves` is the initial position's slot
/// (R-JKF-001).
fn normalize_moves(
    moves: &mut [MoveFormat],
    first_ply: usize,
    mut pos: PartialPosition,
    mut totals: [TimeFormat; 2],
    how: Normalization,
) -> Result<(), NormalizeError> {
    let mut board = Board::new();
    for (offset, mf) in moves.iter_mut().enumerate() {
        let ply = first_ply + offset;
        // A fork that cannot be normalized is still a fork. A kifu recording
        // an illegal move is valid input (R-RULE-002), and dropping the fork
        // here returns `Ok` with the record one line shorter — the caller saves
        // it back and the variation is gone from the file for good.
        //
        // `populate_relative_moves` keeps them for the same reason.
        //
        // A fork is the alternative *to* this node's move (R-JKF-004), so it
        // starts from the position before it. Once that position is lost there
        // is nothing to normalize a fork against, and going ahead rewrites its
        // moves against a board they never came from — `ColorSource::Position` would
        // give them the wrong side.
        if board.is_known() {
            if let Some(forks) = &mut mf.forks {
                for fork in forks.iter_mut() {
                    // A fork replaces this node, so its first element sits at
                    // the same ply (R-JKF-004).
                    let _ = normalize_moves(fork, ply, pos.clone(), totals, how);
                }
            }
        }
        // The running total is per side, so it needs to know whose turn it is.
        // Once the board is gone that is a guess, and a guessed total overwrites
        // a stated one — every later move lands on the same side's clock.
        if board.is_known() {
            if let Some(time) = &mut mf.time {
                totals[pos.side_to_move().array_index()] =
                    add_timeformat(&totals[pos.side_to_move().array_index()], &time.now);
                time.total = totals[pos.side_to_move().array_index()];
            }
        }
        // A node without a move holds a comment or a special move. Neither ends the
        // record: `中断` appears mid-list in a game that was interrupted and
        // resumed, and a comment can sit between two moves. Stopping here would
        // leave every later move with its parsed colour, its `from` unresolved
        // and its forks unnormalized.
        let Some(mmf) = &mut mf.move_ else {
            board.saw(mf);
            continue;
        };
        if !board.is_known() {
            continue;
        }
        // `normalize_move` rewrites the move from the position before it knows
        // whether the position holds it, so it runs on a copy. Otherwise a move
        // the board turns out not to explain keeps the board's answer: a 銀
        // saved as a 金 because a 金 was the piece on that square in the
        // position *before* the interruption.
        //
        // `PartialPosition::make_move` leaves `pos` alone when it returns
        // `None`, so the guard below is safe to run for its effect.
        let mut candidate = *mmf;
        match normalize_move(&mut candidate, &pos, how) {
            Ok(mv) if pos.make_move(mv).is_some() => *mmf = candidate,
            Ok(mv) => board
                .unplayable(|| NormalizeError::at(ply, NormalizeErrorKind::MakeMoveFailed(mv)))?,
            Err(kind) => board.unplayable(|| NormalizeError::at(ply, kind))?,
        }
    }
    Ok(())
}

/// Fills in `relative`, `first_ply` being the ply `moves[0]` sits at.
fn populate_relative_moves(
    moves: &mut [MoveFormat],
    first_ply: usize,
    mut pos: PartialPosition,
) -> Result<(), NormalizeError> {
    let mut board = Board::new();
    for (offset, mf) in moves.iter_mut().enumerate() {
        let ply = first_ply + offset;
        if board.is_known() {
            if let Some(forks) = &mut mf.forks {
                for v in forks.iter_mut() {
                    // A fork that cannot be replayed is still a fork
                    // (R-RULE-002), the same as in `normalize_moves`. Dropping
                    // it here would lose a variation to fill in a derived field.
                    let _ = populate_relative_moves(v, ply, pos.clone());
                }
            }
        }
        let Some(mmf) = &mut mf.move_ else {
            board.saw(mf);
            continue;
        };
        if !board.is_known() {
            continue;
        }
        let mv = match shogi_core::Move::try_from(&*mmf) {
            Ok(mv) => mv,
            Err(err) => {
                board.unplayable(|| NormalizeError::at(ply, err))?;
                continue;
            }
        };
        // The suffix says which of several pieces made the move, so it is read
        // off the position *before* it — and written only once that position
        // turns out to explain the move at all. A suffix taken from a board the
        // move never came from is a wrong answer written into the record.
        //
        // Only worked out when it will be used. Deriving it regardless costs a
        // scan of all 81 squares per move, and a record that already carries
        // `relative` — which is every record that came from anywhere but a KIF
        // — spends the whole of it on a value that is then thrown away. R-REQ-006
        // took that same waste out of the KIF path; this is the same waste in
        // the function that fills the field in.
        let suffix = mmf
            .relative
            .is_none()
            .then(|| infer_relative_from_position(&pos, mv));
        if pos.make_move(mv).is_none() {
            board.unplayable(|| NormalizeError::at(ply, NormalizeErrorKind::MakeMoveFailed(mv)))?;
            continue;
        }
        if let Some(suffix) = suffix {
            mmf.relative = match suffix {
                Suffix::Only(relative) => Some(relative),
                Suffix::Nothing | Suffix::Unspellable => None,
            };
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
