//! How a square and a piece are spelled.
//!
//! The traditional notation writes a square as its file in full-width digits
//! and its rank in kanji — `７六` (R-NOT-001). Every format this crate reads
//! and writes uses that spelling, and so does anything the crate says about a
//! record it could not process: `shogi_core::Square`'s own `Debug` prints the
//! internal index (`Square(61)`), which is not a shogi coordinate and cannot be
//! looked up in the file the reader was given.
//!
//! One table, not one per caller. A writer and an error message that disagree
//! about which character a rank is point a reader at the wrong line.

use crate::jkf::Kind;

/// Files, as the full-width digits `１`-`９`.
pub(crate) const SANYOU_SUJI: [char; 9] = ['１', '２', '３', '４', '５', '６', '７', '８', '９'];

/// Numbers 1-10 in kanji. Ranks use 1-9; hand counts reach 18 via `十`.
pub(crate) const KANSUJI: [char; 10] = ['一', '二', '三', '四', '五', '六', '七', '八', '九', '十'];

/// The character a board diagram and a move both give `kind` (R-NOT-006).
///
/// This is the *board* spelling, in which a promoted piece is one character
/// (`杏` `圭` `全`). KIF's move text may instead write it out as `成香`, which
/// is a different table and lives with the writer that needs it (R-KI2-005).
pub(crate) const fn board_word(kind: Kind) -> char {
    match kind {
        Kind::FU => '歩',
        Kind::KY => '香',
        Kind::KE => '桂',
        Kind::GI => '銀',
        Kind::KI => '金',
        Kind::KA => '角',
        Kind::HI => '飛',
        Kind::OU => '玉',
        Kind::TO => 'と',
        Kind::NY => '杏',
        Kind::NK => '圭',
        Kind::NG => '全',
        Kind::UM => '馬',
        Kind::RY => '龍',
    }
}

/// The JKF name for a [`shogi_core::PieceKind`].
pub(crate) const fn pk2k(pk: shogi_core::PieceKind) -> Kind {
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
