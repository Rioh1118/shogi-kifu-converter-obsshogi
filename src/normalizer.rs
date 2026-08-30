use crate::error::{ConvertError, NormalizeError};
use crate::jkf::*;
use shogi_core::PartialPosition;
use shogi_legality_lite::prelegality::is_valid;

pub(crate) const HIRATE_BOARD: [[Piece; 9]; 9] = {
    #[rustfmt::skip]
    const EMP: Piece = Piece { color: None, kind: None };
    #[rustfmt::skip]
    const BFU: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::FU) };
    #[rustfmt::skip]
    const BKY: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::KY) };
    #[rustfmt::skip]
    const BKE: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::KE) };
    #[rustfmt::skip]
    const BGI: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::GI) };
    #[rustfmt::skip]
    const BKI: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::KI) };
    #[rustfmt::skip]
    const BKA: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::KA) };
    #[rustfmt::skip]
    const BHI: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::HI) };
    #[rustfmt::skip]
    const BOU: Piece = Piece { color: Some(Color::Black), kind: Some(Kind::OU) };
    #[rustfmt::skip]
    const WFU: Piece = Piece { color: Some(Color::White), kind: Some(Kind::FU) };
    #[rustfmt::skip]
    const WKY: Piece = Piece { color: Some(Color::White), kind: Some(Kind::KY) };
    #[rustfmt::skip]
    const WKE: Piece = Piece { color: Some(Color::White), kind: Some(Kind::KE) };
    #[rustfmt::skip]
    const WGI: Piece = Piece { color: Some(Color::White), kind: Some(Kind::GI) };
    #[rustfmt::skip]
    const WKI: Piece = Piece { color: Some(Color::White), kind: Some(Kind::KI) };
    #[rustfmt::skip]
    const WKA: Piece = Piece { color: Some(Color::White), kind: Some(Kind::KA) };
    #[rustfmt::skip]
    const WHI: Piece = Piece { color: Some(Color::White), kind: Some(Kind::HI) };
    #[rustfmt::skip]
    const WOU: Piece = Piece { color: Some(Color::White), kind: Some(Kind::OU) };
    [
        [WKY, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BKY],
        [WKE, WKA, WFU, EMP, EMP, EMP, BFU, BHI, BKE],
        [WGI, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BGI],
        [WKI, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BKI],
        [WOU, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BOU],
        [WKI, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BKI],
        [WGI, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BGI],
        [WKE, WHI, WFU, EMP, EMP, EMP, BFU, BKA, BKE],
        [WKY, EMP, WFU, EMP, EMP, EMP, BFU, EMP, BKY],
    ]
};

impl Piece {
    pub(crate) const fn empty() -> Self {
        Self {
            color: None,
            kind: None,
        }
    }
}

impl Kind {
    pub(crate) fn promoted(self) -> Self {
        match self {
            Kind::FU => Kind::TO,
            Kind::KY => Kind::NY,
            Kind::KE => Kind::NK,
            Kind::GI => Kind::NG,
            Kind::KA => Kind::UM,
            Kind::HI => Kind::RY,
            _ => self,
        }
    }
    pub(crate) fn unpromoted(self) -> Self {
        match self {
            Kind::TO => Kind::FU,
            Kind::NY => Kind::KY,
            Kind::NK => Kind::KE,
            Kind::NG => Kind::GI,
            Kind::UM => Kind::KA,
            Kind::RY => Kind::HI,
            _ => self,
        }
    }
}

impl Hand {
    pub(crate) const fn empty() -> Self {
        Hand {
            FU: 0,
            KY: 0,
            KE: 0,
            GI: 0,
            KI: 0,
            KA: 0,
            HI: 0,
        }
    }
    /// The slot for `kind`, or `None` for a king or a promoted piece.
    ///
    /// A king never goes to the hand (R-CSA-006), and a promoted piece is turned
    /// back over when captured, so neither can sit in one. A broken CSA can
    /// still say they do, and that has to come back as an error rather than
    /// take the process down.
    fn slot(&mut self, kind: Kind) -> Option<&mut u8> {
        Some(match kind {
            Kind::FU => &mut self.FU,
            Kind::KY => &mut self.KY,
            Kind::KE => &mut self.KE,
            Kind::GI => &mut self.GI,
            Kind::KI => &mut self.KI,
            Kind::KA => &mut self.KA,
            Kind::HI => &mut self.HI,
            _ => return None,
        })
    }
    /// Adds one to `kind`. `None` if it cannot be held, or would overflow.
    pub(crate) fn increment(&mut self, kind: Kind) -> Option<()> {
        let slot = self.slot(kind)?;
        *slot = slot.checked_add(1)?;
        Some(())
    }
    /// Takes one from `kind`. `None` if it cannot be held, or none are left —
    /// which means the board holds more of that piece than a set contains.
    pub(crate) fn decrement(&mut self, kind: Kind) -> Option<()> {
        let slot = self.slot(kind)?;
        *slot = slot.checked_sub(1)?;
        Some(())
    }
}

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
    // Widen before adding: two times that each fit in a `u8` need not.
    let total = (lhs.h.unwrap_or_default() as u64 + rhs.h.unwrap_or_default() as u64) * 3600
        + (lhs.m as u64 + rhs.m as u64) * 60
        + (lhs.s as u64 + rhs.s as u64);
    TimeFormat {
        // Hours past 255 saturate rather than wrap. A cumulative time that long
        // is broken input, and a wrapped value would be a plausible-looking lie.
        h: Some((total / 3600).min(u8::MAX as u64) as u8),
        m: ((total / 60) % 60) as u8,
        s: (total % 60) as u8,
    }
}

fn pk2k(pk: shogi_core::PieceKind) -> Kind {
    match pk {
        shogi_core::PieceKind::Pawn => Kind::FU,
        shogi_core::PieceKind::Lance => Kind::KY,
        shogi_core::PieceKind::Knight => Kind::KE,
        shogi_core::PieceKind::Silver => Kind::GI,
        shogi_core::PieceKind::Gold => Kind::KI,
        shogi_core::PieceKind::Bishop => Kind::KA,
        shogi_core::PieceKind::Rook => Kind::HI,
        shogi_core::PieceKind::King => Kind::OU,
        shogi_core::PieceKind::ProPawn => Kind::TO,
        shogi_core::PieceKind::ProLance => Kind::NY,
        shogi_core::PieceKind::ProKnight => Kind::NK,
        shogi_core::PieceKind::ProSilver => Kind::NG,
        shogi_core::PieceKind::ProBishop => Kind::UM,
        shogi_core::PieceKind::ProRook => Kind::RY,
    }
}

impl JsonKifuFormat {
    /// Normalizes the JKF data.
    ///
    /// If `correct_color` is true, the color of each move is corrected from the position's
    /// side to move (for KIF/KI2 where color is assigned by move number parity and may be wrong).
    /// If false, a color mismatch is treated as an error (for CSA/JKF where color is reliable).
    /// If `infer_relative` is false, the slow `relative` (左/右/上/...) inference is skipped.
    /// KIF parsing supplies an explicit `from`, so the inference is dead work for that path.
    /// Downstream code that needs `relative` (e.g. KI2 conversion) can call
    /// [`Self::populate_relative`] to fill it in lazily.
    pub fn normalize_with_options(
        &mut self,
        correct_color: bool,
        infer_relative: bool,
    ) -> Result<(), NormalizeError> {
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
            match PartialPosition::try_from(initial) {
                Ok(pos) => pos,
                Err(err) => return Err(NormalizeError::Convert(err.to_string())),
            }
        } else {
            PartialPosition::startpos()
        };
        let (_, rest) = match self.moves.split_first_mut() {
            Some(split) => split,
            // Index 0 is the initial position's comments, so an empty `moves`
            // has no plies to normalize rather than being an error.
            None => return Ok(()),
        };
        normalize_moves(
            rest,
            pos,
            [TimeFormat::default(); 2],
            correct_color,
            infer_relative,
        )?;
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
    pub fn normalize_with_color_correction(
        &mut self,
        correct_color: bool,
    ) -> Result<(), NormalizeError> {
        self.normalize_with_options(correct_color, true)
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
    /// The following fields are recomputed from the position and overwrite
    /// whatever the input held: `piece`, `same`, `promote`, `capture` and
    /// `time.total`.
    ///
    /// # Errors
    ///
    /// Returns [`NormalizeError`] if a move cannot be resolved against the
    /// position it is played from.
    pub fn normalize(&mut self) -> Result<(), NormalizeError> {
        self.normalize_with_options(false, true)
    }

    /// The position the moves start from, or `None` if `initial` cannot be
    /// turned into one.
    pub(crate) fn starting_position(&self) -> Option<PartialPosition> {
        match &self.initial {
            Some(initial) => PartialPosition::try_from(initial).ok(),
            None => Some(PartialPosition::startpos()),
        }
    }

    /// Fills in `relative` (左/右/上/...) for every move whose `relative` is `None`,
    /// re-simulating the position from the initial state. Use this after parsing a KIF
    /// (which skips the inference for speed) when a downstream consumer needs `relative`
    /// — e.g. KI2 conversion.
    pub fn populate_relative(&mut self) -> Result<(), NormalizeError> {
        let pos = if let Some(initial) = &self.initial {
            match PartialPosition::try_from(initial) {
                Ok(pos) => pos,
                Err(err) => return Err(NormalizeError::Convert(err.to_string())),
            }
        } else {
            PartialPosition::startpos()
        };
        match self.moves.split_first_mut() {
            Some((_, rest)) => populate_relative_moves(rest, pos),
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

// Check if the `from` is retrievable from the position
fn calculate_from(
    mmf: &MoveMoveFormat,
    pos: &PartialPosition,
    to: shogi_core::Square,
) -> Result<Option<PlaceFormat>, NormalizeError> {
    let color = pos.side_to_move();
    // The same candidate set the writer uses (`infer_relative_from_position`).
    // `LiteLegalityChecker::normal_to_candidates` is not that set: it tests full
    // legality, so a pinned piece drops out, while the writer follows
    // `shogi_official_kifu` and counts it. Reading a suffix with a different
    // set than the one that wrote it is how a move comes back ambiguous.
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
                .ok_or_else(|| NormalizeError::AmbiguousMoveFrom(all.clone()))?;
            // Ask which candidate the writer would have spelled this way. Any
            // other reading of the suffix is a second copy of R-NOT-004, and the
            // two copies drift: 左/右 were fixed on this side once and 直 was
            // left behind, so a `▲５八金直` this crate wrote came back ambiguous.
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
                Err(NormalizeError::AmbiguousMoveFrom(froms))
            }
        }
    }
}

fn normalize_move(
    mmf: &mut MoveMoveFormat,
    pos: &PartialPosition,
    correct_color: bool,
    infer_relative: bool,
) -> Result<shogi_core::Move, NormalizeError> {
    if correct_color {
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
        return Err(NormalizeError::InvalidColor);
    }
    if mmf.same.is_some() {
        mmf.to = pos
            .last_move()
            .map(|mv| PlaceFormat {
                x: mv.to().file(),
                y: mv.to().rank(),
            })
            .ok_or(NormalizeError::NoLastMove)?;
    }
    let to = match shogi_core::Square::try_from(&mmf.to) {
        Ok(to) => to,
        Err(err) => return Err(NormalizeError::Convert(err.to_string())),
    };
    if mmf.from == Some(ORIGIN_UNSTATED) {
        mmf.from = calculate_from(mmf, pos, to)?;
    }
    if let Some(pf) = &mmf.from {
        if let Ok(from) = pf.try_into() {
            // Retrieve piece
            let piece = match pos.piece_at(from) {
                Some(piece) => piece,
                None => return Err(NormalizeError::NoPieceAt(from)),
            };
            let from_piece_kind = piece.piece_kind();
            let to_piece_kind = {
                let pk = shogi_core::PieceKind::from(mmf.piece);
                if mmf.promote.unwrap_or_default() {
                    pk.promote().unwrap_or(pk)
                } else {
                    pk
                }
            };
            mmf.piece = pk2k(from_piece_kind);
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
            // Set promote?
            mmf.promote = if from_piece_kind.promote().is_some()
                && (from.relative_rank(pos.side_to_move()) <= 3
                    || to.relative_rank(pos.side_to_move()) <= 3)
            {
                Some(from_piece_kind != to_piece_kind)
            } else {
                None
            };
            // Set capture?
            mmf.capture = pos.piece_at(to).map(|p| pk2k(p.piece_kind()));
        } else {
            // An origin off the board is a coordinate we could not read, not a
            // statement that the piece came from the hand. JKF says a drop by
            // leaving `from` out (R-JKF-003), and turning one into the other
            // loses the square for good — the writers reject the same value
            // rather than spell `(00)`, which KIF has no meaning for
            // (R-KIF-005).
            return Err(NormalizeError::Convert(
                ConvertError::InvalidSquare((pf.x, pf.y)).to_string(),
            ));
        }
    }
    let mv = match shogi_core::Move::try_from(&*mmf) {
        Ok(mv) => mv,
        Err(err) => return Err(NormalizeError::Convert(err.to_string())),
    };
    // Set relative?
    if infer_relative && mmf.relative.is_none() {
        // A move the notation cannot spell has no suffix to record; the writer
        // is where that has to become an error, not the JKF field.
        mmf.relative = match infer_relative_from_position(pos, mv) {
            Suffix::Only(relative) => Some(relative),
            Suffix::Nothing | Suffix::Unspellable => None,
        };
    }
    Ok(mv)
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

fn normalize_moves(
    moves: &mut [MoveFormat],
    mut pos: PartialPosition,
    mut totals: [TimeFormat; 2],
    correct_color: bool,
    infer_relative: bool,
) -> Result<(), NormalizeError> {
    // Whether an outcome has gone by, and whether the board is still known.
    let (mut after_outcome, mut position_known) = (false, true);
    for mf in moves {
        // A branch that cannot be normalized is still a branch. A kifu recording
        // an illegal move is valid input (R-RULE-002), and dropping the branch
        // here returns `Ok` with the record one line shorter — the caller saves
        // it back and the variation is gone from the file for good.
        //
        // `populate_relative_moves` keeps them for the same reason.
        //
        // A branch is the alternative *to* this node's move (R-JKF-004), so it
        // starts from the position before it. Once that position is lost there
        // is nothing to normalize a branch against, and going ahead rewrites its
        // moves against a board they never came from — `correct_color` would
        // give them the wrong side.
        if position_known {
            if let Some(forks) = &mut mf.forks {
                for fork in forks.iter_mut() {
                    let _ =
                        normalize_moves(fork, pos.clone(), totals, correct_color, infer_relative);
                }
            }
        }
        // The running total is per side, so it needs to know whose turn it is.
        // Once the board is gone that is a guess, and a guessed total overwrites
        // a stated one — every later move lands on the same side's clock.
        if position_known {
            if let Some(time) = &mut mf.time {
                totals[pos.side_to_move().array_index()] =
                    add_timeformat(&totals[pos.side_to_move().array_index()], &time.now);
                time.total = totals[pos.side_to_move().array_index()];
            }
        }
        // A node without a move holds a comment or an outcome. Neither ends the
        // record: `中断` appears mid-list in a game that was interrupted and
        // resumed, and a comment can sit between two moves. Stopping here would
        // leave every later move with its parsed colour, its `from` unresolved
        // and its branches unnormalized.
        let Some(mmf) = &mut mf.move_ else {
            // What follows an outcome need not continue the position before it
            // — a game that was interrupted and resumed picks up from wherever
            // it left off — so a move that will not apply there is not a broken
            // record (R-RULE-002). The board is just no longer ours to track.
            after_outcome |= mf.special.is_some();
            continue;
        };
        if !position_known {
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
        match normalize_move(&mut candidate, &pos, correct_color, infer_relative) {
            Ok(mv) if pos.make_move(mv).is_some() => *mmf = candidate,
            _ if after_outcome => position_known = false,
            Ok(mv) => return Err(NormalizeError::MakeMoveFailed(mv)),
            Err(err) => return Err(err),
        }
    }
    Ok(())
}

fn populate_relative_moves(
    moves: &mut [MoveFormat],
    mut pos: PartialPosition,
) -> Result<(), NormalizeError> {
    for mf in moves {
        if let Some(forks) = &mut mf.forks {
            for v in forks.iter_mut() {
                // A branch that cannot be replayed is still a branch
                // (R-RULE-002), the same as in `normalize_moves`. Dropping it
                // here would lose a variation to fill in a derived field.
                let _ = populate_relative_moves(v, pos.clone());
            }
        }
        if let Some(mmf) = &mut mf.move_ {
            let mv = match shogi_core::Move::try_from(&*mmf) {
                Ok(mv) => mv,
                Err(err) => return Err(NormalizeError::Convert(err.to_string())),
            };
            if mmf.relative.is_none() {
                mmf.relative = match infer_relative_from_position(&pos, mv) {
                    Suffix::Only(relative) => Some(relative),
                    Suffix::Nothing | Suffix::Unspellable => None,
                };
            }
            if pos.make_move(mv).is_none() {
                return Err(NormalizeError::MakeMoveFailed(mv));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    // `同` carries no destination of its own — it means the square the previous
    // move went to (R-NOT-002) — and only normalization can fill it in. Stopping
    // at the outcome node leaves `to` at (0,0), which no writer can spell, so
    // saving a game that was interrupted and resumed fails outright.
    #[test]
    fn a_move_after_an_interruption_is_still_normalized() {
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２二角成(88)
   4 中断
   5 同　銀(31)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        let mv = jkf.moves[5].move_.expect("a move");
        assert_eq!(PlaceFormat { x: 2, y: 2 }, mv.to, "`同` resolves to 2二");
        assert_eq!(Color::White, mv.color);
        assert_eq!(Some(Kind::UM), mv.capture, "it takes the horse");
        // And the record can be written back out.
        use crate::converter::{ToKi2, ToKif};
        assert!(jkf.try_to_kif_owned().is_ok());
        assert!(jkf.try_to_ki2_owned().is_ok());
    }

    // `直` is the only way to read `▲５八金直` when more than one gold reaches
    // the square, and it is the one arm of `calculate_from`'s match that the
    // 左/右 rewrite did not touch. Left unfiltered, the move comes back ambiguous
    // and the branch holding it is dropped.
    #[test]
    fn a_gold_moving_straight_up_is_read_from_直() {
        let kif = "手合割：その他
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
| 玉 ・ ・ 金 金 金 ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 ５八金(59)
";
        use crate::converter::ToKi2;
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains("▲５八金直"), "{ki2:?}");
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(
            Some(PlaceFormat { x: 5, y: 9 }),
            back.moves[1].move_.expect("a move").from,
        );
    }

    /// Builds a board holding `pieces`, plus a king for each side, and reads
    /// the KIF for `mv` played from it.
    fn from_board(pieces: &[(usize, usize, Color, Kind)], mv: &str) -> JsonKifuFormat {
        let mut board = String::new();
        let mut cells = [[None; 9]; 9];
        // Kings in the corners, out of the way of the squares these tests use.
        cells[0][8] = Some((Color::Black, Kind::OU));
        cells[0][0] = Some((Color::White, Kind::OU));
        for &(x, y, color, kind) in pieces {
            cells[x - 1][y - 1] = Some((color, kind));
        }
        for rank in 1..=9usize {
            board.push('|');
            for file in (1..=9usize).rev() {
                match cells[file - 1][rank - 1] {
                    None => board.push_str(" ・"),
                    Some((color, kind)) => {
                        board.push(if color == Color::Black { ' ' } else { 'v' });
                        board.push_str(match kind {
                            Kind::KI => "金",
                            Kind::GI => "銀",
                            Kind::TO => "と",
                            Kind::KA => "角",
                            Kind::UM => "馬",
                            Kind::HI => "飛",
                            Kind::RY => "龍",
                            Kind::OU => "玉",
                            _ => unreachable!("only the kinds these tests place"),
                        });
                    }
                }
            }
            board.push('|');
            board.push_str(["一", "二", "三", "四", "五", "六", "七", "八", "九"][rank - 1]);
            board.push('\n');
        }
        let kif = format!(
            "手合割：その他\n後手の持駒：なし\n  ９ ８ ７ ６ ５ ４ ３ ２ １\n\
+---------------------------+\n{board}+---------------------------+\n\
先手の持駒：なし\n先手番\n手数----指手---------消費時間--\n   1 {mv}\n"
        );
        crate::parser::parse_kif_str(&kif).unwrap_or_else(|e| panic!("{kif}\n{e}"))
    }

    // R-NOT-004 stage 2: 直 is for a gold-like piece going straight *up*. A gold
    // directly behind the destination is not 直 — it would be 引 — so a reader
    // that only checks the file keeps it as a candidate and the move comes back
    // ambiguous. The writer already knew; only the reader did not, which is what
    // a second copy of the rule buys.
    //
    // 5七 / 5九 / 4九 all reach 5八, and the move is 5九→5八.
    #[test]
    fn a_gold_behind_the_destination_is_not_直() {
        use crate::converter::ToKi2;
        let jkf = from_board(
            &[
                (5, 7, Color::Black, Kind::KI),
                (5, 9, Color::Black, Kind::KI),
                (4, 9, Color::Black, Kind::KI),
            ],
            "５八金(59)",
        );
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains("▲５八金直"), "{ki2:?}");
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(
            Some(PlaceFormat { x: 5, y: 9 }),
            back.moves[1].move_.expect("a move").from,
        );
    }

    // Every arrangement of gold-like pieces around one square, written and read
    // back. Enumeration is what found the 直 case above: it is not a shape
    // anyone thinks to write by hand, and the corpus has none of it.
    //
    // Either the notation can name the mover — and then reading has to land on
    // the same square — or it cannot, and then the write has to fail rather than
    // produce a record that will not open (R-NOT-004 stage 3, R-KI2-003).
    #[test]
    fn every_arrangement_of_golds_round_trips_or_refuses() {
        use crate::converter::ToKi2;
        // The squares a Black gold reaches 5八 from.
        const AROUND: [(usize, usize); 6] = [(5, 7), (4, 8), (6, 8), (4, 9), (6, 9), (5, 9)];
        let mut spelled = 0;
        let mut refused = 0;
        for kind in [Kind::KI, Kind::TO] {
            for mask in 0u32..(1 << AROUND.len()) {
                let placed: Vec<_> = AROUND
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| mask & (1 << i) != 0)
                    .map(|(_, &(x, y))| (x, y, Color::Black, kind))
                    .collect();
                if placed.len() < 2 {
                    continue;
                }
                for &(x, y, ..) in &placed {
                    let jkf = from_board(&placed, &format!("５八{}({x}{y})", kif_word(kind)));
                    let Ok(ki2) = jkf.try_to_ki2_owned() else {
                        refused += 1;
                        continue;
                    };
                    spelled += 1;
                    let back = crate::parser::parse_ki2_str(&ki2).unwrap_or_else(|e| {
                        panic!("wrote {ki2:?} from {placed:?} and could not read it: {e}")
                    });
                    assert_eq!(
                        Some(PlaceFormat {
                            x: x as u8,
                            y: y as u8
                        }),
                        back.moves[1].move_.expect("a move").from,
                        "{ki2:?} from {placed:?}"
                    );
                }
            }
        }
        // Gold-like pieces reach a square from at most six others, and 左/直/右
        // crossed with 上/寄/引 names nine — so every arrangement of them is
        // spellable. The unspellable case needs a piece that comes from further
        // away; `a_move_the_notation_cannot_spell_is_not_written` has it.
        assert_eq!(0, refused, "a gold arrangement could not be spelled");
        assert!(spelled > 100, "only {spelled} arrangements were exercised");
    }

    fn kif_word(kind: Kind) -> &'static str {
        match kind {
            Kind::KI => "金",
            Kind::TO => "と",
            _ => unreachable!("only the kinds this test places"),
        }
    }

    // R-NOT-004 stage 3: three bishops reaching one square cannot be told apart
    // — the notation has 左 and 右 and nothing else for them. Writing the move
    // bare produces KI2 nothing can read (R-KI2-003), and the record being saved
    // is the only copy, so the write has to fail instead.
    #[test]
    fn a_move_the_notation_cannot_spell_is_not_written() {
        use crate::converter::ToKi2;
        let jkf = from_board(
            &[
                (1, 7, Color::Black, Kind::KA),
                (3, 1, Color::Black, Kind::KA),
                (6, 2, Color::Black, Kind::KA),
            ],
            "５三角(31)",
        );
        assert!(
            jkf.try_to_ki2_owned().is_err(),
            "three bishops reach 5三: {:?}",
            jkf.try_to_ki2_owned()
        );
    }

    // A branch whose moves cannot be replayed is still a branch. Dropping it
    // returns `Ok` with the record one variation shorter, and the next save
    // writes the shortened record over the original — the loss is permanent and
    // nothing ever said so. Kifu holding illegal moves are valid input
    // (R-RULE-002).
    //
    // The moves after `中断` here do not continue the position before it, which
    // is what a resumed game looks like.
    #[test]
    fn a_branch_that_cannot_be_replayed_is_kept() {
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２六歩(27)
   4 ８四歩(83)

変化：2手
   2 ８四歩(83)
   3 中断
   4 ９九角(88)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        let forks = jkf.moves[2].forks.as_ref().expect("a branch at ply 2");
        assert_eq!(1, forks.len(), "the branch survives normalization");
        assert_eq!(3, forks[0].len(), "and keeps all three of its nodes");
    }

    // JKF says a drop by leaving `from` out (R-JKF-003), so an origin that is
    // not a square on the board cannot be quietly turned into one — the square
    // is lost and the record comes back saying the piece came from the hand.
    // `(00)` is CSA's spelling for a drop, so it is a shape a converter written
    // by someone else really does produce.
    #[test]
    fn an_origin_off_the_board_is_an_error_not_a_drop() {
        for origin in ["(00)", "(10)", "(09)"] {
            let kif =
                format!("手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩{origin}\n");
            assert!(
                crate::parser::parse_kif_str(&kif).is_err(),
                "{origin} was accepted"
            );
        }
    }

    // R-KIF-008: the move's own time is 分:秒, and the minutes have no stated
    // limit. An hour has to fold into them — writing only `time.now.m` drops it
    // and leaves the file disagreeing with its own running total.
    #[test]
    fn an_hour_of_thinking_survives_a_kif_round_trip() {
        use crate::converter::ToKif;
        let csa = "V2.2\nPI\n+\n+7776FU\nT3723\n";
        let jkf = crate::parser::parse_csa_str(csa).expect("parses CSA");
        assert_eq!(
            Some(1),
            jkf.moves[1].time.expect("a time").now.h,
            "3723 seconds is an hour and change"
        );
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        assert!(kif.contains("(62:03/01:02:03)"), "{kif:?}");
        let back = crate::parser::parse_kif_str(&kif).expect("reads back");
        let now = back.moves[1].time.expect("a time").now;
        assert_eq!(
            3723,
            u32::from(now.h.unwrap_or_default()) * 3600 + u32::from(now.m) * 60 + u32::from(now.s),
            "the seconds spent on the move"
        );
    }

    // A move the board turns out not to explain keeps whatever it said. The
    // normalization runs before anyone knows the board holds the move, so it has
    // to run on a copy: otherwise a 銀 comes back as the 金 that stood on that
    // square in the position *before* the interruption, and the running clock
    // keeps adding to one side.
    #[test]
    fn a_move_the_board_cannot_explain_is_left_alone() {
        // 6九 holds a gold at the start, so normalizing 6八銀(69) against the
        // pre-中断 board would rewrite the piece.
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77) ( 0:10/00:00:10)
   2 中断
   3 ６八銀(69) ( 0:20/00:00:20)
   4 ３四歩(33) ( 0:30/00:00:30)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        assert_eq!(
            Kind::GI,
            jkf.moves[3].move_.expect("a move").piece,
            "the piece the file named"
        );
        for (i, want) in [(3, 20), (4, 30)] {
            assert_eq!(
                TimeFormat {
                    h: Some(0),
                    m: 0,
                    s: want
                },
                jkf.moves[i].time.expect("a time").total,
                "the total the file stated at ply {i}"
            );
        }
    }

    // The board is dropped at the *first* move an outcome's position cannot
    // explain, and stays dropped. Without that, every later move is normalized
    // against a board the game left behind.
    #[test]
    fn the_board_stays_dropped_after_an_outcome() {
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
   4 ６八銀(69)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        assert_eq!(
            Kind::GI,
            jkf.moves[4].move_.expect("a move").piece,
            "the move after the one that lost the board is untouched too"
        );
    }

    // A branch hanging off a node past the point the board was lost has no
    // position to be normalized against either. Normalizing it anyway rewrites
    // its moves — `correct_color` alone flips the side — against a board they
    // never came from.
    #[test]
    fn a_branch_past_the_lost_board_is_left_alone() {
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
   4 ６八銀(69)

変化：4手
   4 ２六歩(27)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        let fork = &jkf.moves[4].forks.as_ref().expect("a branch")[0];
        let mv = fork[0].move_.expect("a move");
        assert_eq!(Color::White, mv.color, "the side the ply number gave it");
        assert_eq!(Some(PlaceFormat { x: 2, y: 7 }), mv.from);
    }

    // R-RULE-002: a kifu recording an illegal move is valid input, and
    // `is_valid` is exactly what leaves such a move out of the candidate scan.
    // Left out, the suffix describes one of the *other* pieces, so the record
    // comes back saying a piece moved that never did.
    //
    // KI2 carries no origin (R-KI2-003), so reading an illegal move back to its
    // square is not possible either way — what the suffix must not do is name
    // the wrong piece.
    #[test]
    fn an_illegal_move_is_spelled_for_the_piece_that_made_it() {
        use crate::converter::ToKi2;
        // 4九 cannot reach 5五; the record says it did. 5六 can, and would be
        // spelled with no suffix at all if it were the only candidate.
        let jkf = from_board(
            &[
                (4, 9, Color::Black, Kind::KI),
                (5, 6, Color::Black, Kind::KI),
            ],
            "５五金(49)",
        );
        let ki2 = jkf
            .try_to_ki2_owned()
            .expect("an illegal move is still writable");
        assert!(ki2.contains("▲５五金右"), "{ki2:?}");
    }

    // The other half: when no legal candidate shares the illegal move's motion,
    // leaving it out of the set makes the notation unable to name anything — and
    // since that is now an error, one illegal move would stop the whole record
    // from being saved.
    #[test]
    fn an_illegal_move_does_not_stop_the_record_from_being_written() {
        use crate::converter::ToKi2;
        let jkf = from_board(
            &[
                (5, 9, Color::Black, Kind::KI),
                (4, 5, Color::Black, Kind::KI),
                (6, 5, Color::Black, Kind::KI),
            ],
            "５五金(59)",
        );
        assert!(
            jkf.try_to_ki2_owned().is_ok(),
            "one illegal move must not cost the whole record"
        );
    }

    // Same shape on the main line: a game that resumed after `中断` and went on
    // from somewhere else. Refusing the whole record would take a readable kifu
    // out of the consumer's index entirely.
    #[test]
    fn a_resumed_game_that_diverges_is_still_readable() {
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        assert_eq!(4, jkf.moves.len(), "every node is kept: {:?}", jkf.moves);
    }

    // JKF says a move is a drop by leaving `from` out (R-JKF-003). Reading that
    // as "the origin is missing, look it up" turns the drop into whichever board
    // piece could have reached the square: that piece comes off the board, the
    // hand is never spent, and a few moves later the square it left is empty.
    //
    // Only KI2 has moves whose origin the notation does not carry, and its
    // reader says so with `ORIGIN_UNSTATED`.
    #[test]
    fn a_drop_stays_a_drop_through_normalize() {
        // Black has a gold on 4九 that reaches 3八, and a gold in hand. The
        // record drops the one in hand; the one on the board must not move.
        let mut state = StateFormat {
            color: Color::Black,
            board: [[Piece {
                color: None,
                kind: None,
            }; 9]; 9],
            hands: [Hand::default(); 2],
        };
        let put = |board: &mut [[Piece; 9]; 9], x: usize, y: usize, color, kind| {
            board[x - 1][y - 1] = Piece {
                color: Some(color),
                kind: Some(kind),
            };
        };
        put(&mut state.board, 5, 9, Color::Black, Kind::OU);
        put(&mut state.board, 4, 9, Color::Black, Kind::KI);
        put(&mut state.board, 5, 1, Color::White, Kind::OU);
        state.hands[0].KI = 1;

        let mut jkf = JsonKifuFormat {
            initial: Some(Initial {
                preset: Preset::PresetOther,
                data: Some(state),
            }),
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: None,
                        to: PlaceFormat { x: 3, y: 8 },
                        piece: Kind::KI,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        jkf.normalize().expect("normalizes");
        assert_eq!(
            None,
            jkf.moves[1].move_.expect("a move").from,
            "the drop was rewritten as a board move"
        );
        // And the board gold is still on 4九, not on 3八.
        let pos = jkf.starting_position().expect("a position");
        let mut pos = pos;
        pos.make_move(shogi_core::Move::try_from(&jkf.moves[1].move_.unwrap()).expect("a move"))
            .expect("the drop applies");
        assert!(
            pos.piece_at(shogi_core::Square::new(4, 9).expect("a square"))
                .is_some(),
            "the gold on 4九 must still be there"
        );
    }

    // Cumulative times are summed from values the file states, so they can pass
    // what a `u8` holds. Wrapping would put a small, ordinary-looking number
    // where a broken one belongs, and nothing downstream could tell.
    #[test]
    fn a_cumulative_time_past_the_hour_limit_does_not_wrap() {
        // Black plays twice at 255 hours each: 510, which is 254 once it wraps.
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77) (255:00:00/255:00:00)
   2 ３四歩(33) ( 0:00/00:00:00)
   3 ２六歩(27) (255:00:00/255:00:00)
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        let total = jkf.moves[3].time.expect("a time").total;
        assert_eq!(Some(u8::MAX), total.h, "hours must saturate, not wrap");
    }

    // 馬 reaches a square from that square's own file, so one of two candidates
    // can be named 右 while standing on the destination's file. Comparing the
    // origin with the destination instead of the candidates with each other
    // leaves no candidate and the move reads back as ambiguous — which is how a
    // `.ki2` written by any of the tools in use became unreadable here.
    #[test]
    fn a_horse_on_the_destination_file_still_takes_a_left_or_right() {
        let kif = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ 馬 馬 ・ ・ ・|三
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
   1 ４四馬(43)
";
        use crate::converter::ToKi2;
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains("▲４四馬右"), "wrote {ki2:?}");
        let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(
            Some(PlaceFormat { x: 4, y: 3 }),
            back.moves[1].move_.expect("a move").from,
        );
    }

    /// Two black bishops on 7a and 3a, both able to reach 5c, so every move to
    /// 5c needs a 左/右 disambiguator (R-NOT-004). `{mv}` is the move line.
    fn ambiguous_bishop_kif(mv: &str) -> String {
        format!(
            "手合割：その他
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
   1 {mv}   ( 0:00/00:00:00)
"
        )
    }

    // R-NOT-004 / R-NOT-005: the promotion suffix must not hide the
    // disambiguator. `不成` is covered too — stripping only `成` leaves it
    // broken, and a KI2 written without the disambiguator cannot be read back.
    #[test]
    fn relative_survives_promotion_suffix() {
        for (mv, want) in [
            ("５三角成(71)", Relative::L),
            ("５三角成(31)", Relative::R),
            ("５三角(71)", Relative::L),
            ("５三角(31)", Relative::R),
        ] {
            let src = ambiguous_bishop_kif(mv);
            let mut jkf = crate::parser::parse_kif_str(&src)
                .unwrap_or_else(|e| panic!("failed to parse {mv}: {e}"));
            jkf.populate_relative()
                .unwrap_or_else(|e| panic!("failed to populate {mv}: {e}"));
            assert_eq!(
                Some(want),
                jkf.moves[1].move_.expect("a move").relative,
                "relative for {mv}"
            );
        }
    }

    #[test]
    fn normalize_moves_empty() {
        let pos = PartialPosition::startpos();
        assert!(normalize_moves(&mut [], pos, [TimeFormat::default(); 2], false, true).is_ok());
    }

    #[test]
    fn normalize_moves_invalid_color() {
        let mmf = MoveMoveFormat {
            color: Color::Black,
            piece: Kind::FU,
            from: Some(PlaceFormat { x: 7, y: 7 }),
            to: PlaceFormat { x: 7, y: 6 },
            promote: None,
            capture: None,
            relative: None,
            same: None,
        };
        // Correct color should succeed
        {
            let pos = PartialPosition::startpos();
            assert!(
                normalize_moves(
                    &mut [MoveFormat {
                        move_: Some(mmf),
                        ..Default::default()
                    }],
                    pos,
                    [TimeFormat::default(); 2],
                    false,
                    true,
                )
                .is_ok(),
                "normalize should succeed"
            );
        }
        // Wrong color with correct_color=false should fail
        {
            let pos = PartialPosition::startpos();
            assert!(
                normalize_moves(
                    &mut [MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::White,
                            ..mmf
                        }),
                        ..Default::default()
                    }],
                    pos,
                    [TimeFormat::default(); 2],
                    false,
                    true,
                )
                .is_err(),
                "normalize should fail with InvalidColor"
            );
        }
        // Wrong color with correct_color=true should succeed and fix color
        {
            let pos = PartialPosition::startpos();
            let mut moves = [MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::White,
                    ..mmf
                }),
                ..Default::default()
            }];
            assert!(
                normalize_moves(&mut moves, pos, [TimeFormat::default(); 2], true, true).is_ok(),
                "normalize should succeed (color auto-corrected)"
            );
            assert_eq!(
                moves[0].move_.as_ref().unwrap().color,
                Color::Black,
                "color should be corrected to Black"
            );
        }
    }
}
