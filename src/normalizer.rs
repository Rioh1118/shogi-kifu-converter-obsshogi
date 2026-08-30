use crate::error::NormalizeError;
use crate::jkf::*;
use shogi_core::{LegalityChecker, PartialPosition};
use shogi_legality_lite::prelegality::is_valid;
use shogi_legality_lite::LiteLegalityChecker;
use shogi_official_kifu::display_single_move_kansuji;

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
    pub(crate) fn increment(&mut self, kind: Kind) {
        match kind {
            Kind::FU => self.FU += 1,
            Kind::KY => self.KY += 1,
            Kind::KE => self.KE += 1,
            Kind::GI => self.GI += 1,
            Kind::KI => self.KI += 1,
            Kind::KA => self.KA += 1,
            Kind::HI => self.HI += 1,
            _ => unreachable!(),
        }
    }
    pub(crate) fn decrement(&mut self, kind: Kind) {
        match kind {
            Kind::FU => self.FU -= 1,
            Kind::KY => self.KY -= 1,
            Kind::KE => self.KE -= 1,
            Kind::GI => self.GI -= 1,
            Kind::KI => self.KI -= 1,
            Kind::KA => self.KA -= 1,
            Kind::HI => self.HI -= 1,
            _ => unreachable!(),
        }
    }
}

fn add_timeformat(lhs: &TimeFormat, rhs: &TimeFormat) -> TimeFormat {
    let s = (lhs.h.unwrap_or_default() + rhs.h.unwrap_or_default()) as u64 * 3600
        + (lhs.m + rhs.m) as u64 * 60
        + (lhs.s + rhs.s) as u64;
    let m = (s / 60) % 60;
    let h = s / 3600;
    TimeFormat {
        h: Some(h as u8),
        m: m as u8,
        s: (s % 60) as u8,
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
                for mv in self.moves[1..].iter_mut() {
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
        normalize_moves(
            &mut self.moves[1..],
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
    ///
    /// # Panics
    ///
    /// Panics if `moves` is empty. Index 0 is reserved for the initial
    /// position's comments, so a well-formed value always has one element.
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
    ///
    /// # Panics
    ///
    /// Panics if `moves` is empty. Index 0 is reserved for the initial
    /// position's comments, so a well-formed value always has one element.
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
        populate_relative_moves(&mut self.moves[1..], pos)
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
    let bb = LiteLegalityChecker.normal_to_candidates(
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
            let mut froms: Vec<_> = bb.into_iter().collect();
            let relative = mmf
                .relative
                .ok_or_else(|| NormalizeError::AmbiguousMoveFrom(froms.clone()))?;
            let (to_rel_file, to_rel_rank) = (to.relative_file(color), to.relative_rank(color));
            match relative {
                Relative::L => froms.retain(|sq| sq.relative_file(color) > to_rel_file),
                Relative::C => froms.retain(|sq| sq.file() == to.file()),
                Relative::R => froms.retain(|sq| sq.relative_file(color) < to_rel_file),
                Relative::U => froms.retain(|sq| sq.relative_rank(color) > to_rel_rank),
                Relative::M => froms.retain(|sq| sq.rank() == to.rank()),
                Relative::D => froms.retain(|sq| sq.relative_rank(color) < to_rel_rank),
                Relative::LU => froms.retain(|sq| {
                    sq.relative_file(color) > to_rel_file && sq.relative_rank(color) > to_rel_rank
                }),
                Relative::LM => froms
                    .retain(|sq| sq.relative_file(color) > to_rel_file && sq.rank() == to.rank()),
                Relative::LD => froms.retain(|sq| {
                    sq.relative_file(color) > to_rel_file && sq.relative_rank(color) < to_rel_rank
                }),
                Relative::RU => froms.retain(|sq| {
                    sq.relative_file(color) < to_rel_file && sq.relative_rank(color) > to_rel_rank
                }),
                Relative::RM => froms
                    .retain(|sq| sq.relative_file(color) < to_rel_file && sq.rank() == to.rank()),
                Relative::RD => froms.retain(|sq| {
                    sq.relative_file(color) < to_rel_file && sq.relative_rank(color) < to_rel_rank
                }),
                _ => {}
            };
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
    if mmf.from.is_none() {
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
            mmf.from = None;
        }
    }
    let mv = match shogi_core::Move::try_from(&*mmf) {
        Ok(mv) => mv,
        Err(err) => return Err(NormalizeError::Convert(err.to_string())),
    };
    // Set relative?
    if infer_relative && mmf.relative.is_none() {
        mmf.relative = infer_relative_from_position(pos, mv);
    }
    Ok(mv)
}

/// Infers the disambiguating suffix (左/右/直/上/寄/引/打) for `mv` in `pos`.
///
/// The traditional notation orders a move as
/// `<destination><piece><relative><motion><promotion>` (R-NOT-001), so the
/// promotion suffix has to come off before the relative part is reachable.
/// Reading the tail without stripping it makes every promoting move look like
/// it has no disambiguator, which produces KI2 that cannot be read back
/// (R-NOT-004 / R-NOT-005).
///
/// This is the only place that maps a rendered move back to [`Relative`].
/// Keeping a second copy is what let the promotion bug live in one caller and
/// not the other.
///
/// Rendering is skipped for moves that cannot carry a suffix; see
/// [`needs_disambiguation`].
pub(crate) fn infer_relative_from_position(
    pos: &PartialPosition,
    mv: shogi_core::Move,
) -> Option<Relative> {
    if !needs_disambiguation(pos, mv) {
        return None;
    }
    let mut display = display_single_move_kansuji(pos, mv)?;
    // `不成` has to be tested first: it also ends with `成`.
    let cut = if let Some(rest) = display.strip_suffix("不成") {
        rest.len()
    } else if let Some(rest) = display.strip_suffix('成') {
        rest.len()
    } else {
        display.len()
    };
    display.truncate(cut);
    match (display.pop(), display.pop()) {
        (Some('左'), _) => Some(Relative::L),
        (Some('直'), _) => Some(Relative::C),
        (Some('右'), _) => Some(Relative::R),
        (Some('上'), Some('左')) => Some(Relative::LU),
        (Some('上'), Some('右')) => Some(Relative::RU),
        (Some('上'), _) => Some(Relative::U),
        (Some('引'), Some('左')) => Some(Relative::LD),
        (Some('引'), Some('右')) => Some(Relative::RD),
        (Some('引'), _) => Some(Relative::D),
        (Some('寄'), Some('左')) => Some(Relative::LM),
        (Some('寄'), Some('右')) => Some(Relative::RM),
        (Some('寄'), _) => Some(Relative::M),
        (Some('打'), _) => Some(Relative::H),
        _ => None,
    }
}

/// Whether the traditional notation for `mv` can carry a disambiguating suffix
/// at all.
///
/// `shogi_official_kifu` decides this from the set of squares holding the same
/// piece that could reach the destination: a normal move gets no suffix unless
/// that set has two or more members, and a drop gets `打` only when a board
/// piece could have gone there instead. Answering the question here first is
/// worth the duplication because that crate reaches the answer by enumerating
/// every legal move in the position — about 15,000 candidates — for each single
/// move, while this scan only touches squares that already hold the right piece.
///
/// Uses the same prelegality check the renderer uses. A legality check that
/// also rejects pinned pieces would disagree on a small number of positions and
/// silently change the notation.
fn needs_disambiguation(pos: &PartialPosition, mv: shogi_core::Move) -> bool {
    use shogi_core::{Move, Square};

    let can_reach = |from: Square, to: Square| {
        [false, true]
            .into_iter()
            .any(|promote| is_valid(pos, Move::Normal { from, to, promote }))
    };
    match mv {
        Move::Normal { from, to, .. } => {
            let piece = match pos.piece_at(from) {
                Some(piece) => piece,
                None => return false,
            };
            let mut found = 0;
            for square in Square::all() {
                if pos.piece_at(square) == Some(piece) && can_reach(square, to) {
                    found += 1;
                    if found >= 2 {
                        return true;
                    }
                }
            }
            false
        }
        // A drop is written `打` exactly when the same piece could have been
        // moved to that square instead.
        Move::Drop { to, piece } => {
            let on_board = shogi_core::Piece::new(piece.piece_kind(), pos.side_to_move());
            Square::all()
                .any(|square| pos.piece_at(square) == Some(on_board) && can_reach(square, to))
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
    for mf in moves {
        // Normalize forks (errors in forks are non-fatal; skip invalid ones)
        if let Some(forks) = &mut mf.forks {
            forks.retain_mut(|v| {
                normalize_moves(v, pos.clone(), totals, correct_color, infer_relative).is_ok()
            });
        }
        // Calculate total time
        if let Some(time) = &mut mf.time {
            totals[pos.side_to_move().array_index()] =
                add_timeformat(&totals[pos.side_to_move().array_index()], &time.now);
            time.total = totals[pos.side_to_move().array_index()];
        }
        if let Some(mmf) = &mut mf.move_ {
            let mv = normalize_move(mmf, &pos, correct_color, infer_relative)?;
            if pos.make_move(mv).is_none() {
                return Err(NormalizeError::MakeMoveFailed(mv));
            }
        } else {
            break;
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
                // forks already passed normalization; ignore errors here for parity with retain_mut
                let _ = populate_relative_moves(v, pos.clone());
            }
        }
        if let Some(mmf) = &mut mf.move_ {
            let mv = match shogi_core::Move::try_from(&*mmf) {
                Ok(mv) => mv,
                Err(err) => return Err(NormalizeError::Convert(err.to_string())),
            };
            if mmf.relative.is_none() {
                mmf.relative = infer_relative_from_position(&pos, mv);
            }
            if pos.make_move(mv).is_none() {
                return Err(NormalizeError::MakeMoveFailed(mv));
            }
        } else {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

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
