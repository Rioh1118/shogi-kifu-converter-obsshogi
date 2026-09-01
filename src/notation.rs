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
//!
//! What the notation can and cannot say about a move lives here too, for the
//! same reason: the normalizer and the writers have to answer it the same way.

/// The characters a line ending is made of, whichever of them a file or a value
/// carries.
///
/// R-CSA-001 leaves the newline to the environment, and a JKF built elsewhere
/// carries whatever that environment used — a lone `\r` among them. What the
/// writers do with this is decide what cannot go on one line (a header value,
/// R-KIF-004 / R-CSA-004), and they have to agree with each other: a value one
/// of them splits and another writes through comes back as a header nobody
/// wrote.
///
/// It is not where a reader stops. `parser::kakinoki::end_of_line` is, and it
/// takes what `nom`'s `line_ending` does — `\n` and `\r\n`, not a lone `\r`
/// (`research/90-gaps.md` GAP-027). The readers ask this table whether a
/// character *is* one of those two, which is a narrower question and the same
/// answer either way.
pub(crate) const LINE_ENDS: [char; 2] = ['\n', '\r'];

/// Whether the notation has a `成` / `不成` for this move at all (R-NOT-005).
///
/// A promotable piece, with the enemy camp at one end of the move. A gold, a
/// king or an already-promoted piece has neither word, and neither has a move
/// that goes nowhere near the camp.
///
/// One home, because both directions need it: the normalizer to know whether a
/// move that did not promote is worth recording as `false`, and the KI2 writer
/// to know whether a `false` in the record has a word to be written with. Two
/// copies drift apart, and the writer then spells `△６八玉不成`, which the
/// notation has no word for.
pub(crate) fn promotion_is_spellable(
    piece: shogi_core::PieceKind,
    from: shogi_core::Square,
    to: shogi_core::Square,
    side: shogi_core::Color,
) -> bool {
    piece.promote().is_some() && (from.relative_rank(side) <= 3 || to.relative_rank(side) <= 3)
}

use crate::jkf::Kind;
use std::fmt;

/// Files, as the full-width digits `１`-`９`.
pub(crate) const SANYOU_SUJI: [char; 9] = ['１', '２', '３', '４', '５', '６', '７', '８', '９'];

/// Numbers 1-10 in kanji. Ranks use 1-9; hand counts reach 18 via `十`.
pub(crate) const KANSUJI: [char; 10] = ['一', '二', '三', '四', '五', '六', '七', '八', '九', '十'];

/// The word a move gives `kind` (R-NOT-006).
///
/// The *move* spelling, in which a promoted minor piece is written out
/// (`成香` `成桂` `成銀`) rather than squeezed into the one character a board
/// diagram has room for — see [`board_word`], which is the other table and not
/// this one (R-KI2-005).
///
/// R-NOT-006 says a writer uses the standard form only and a reader takes every
/// variant, so this is one table for both writers. Two of them, hand-written,
/// is how `to_kif` came to write `龍` and `to_ki2` `竜` for the same move: the
/// reader takes either, so nothing catches it, and the consumer saving one game
/// as `.kif` and as `.ki2` gets two different files (R-REQ-002).
pub(crate) const fn move_word(kind: Kind) -> &'static str {
    match kind {
        Kind::FU => "歩",
        Kind::KY => "香",
        Kind::KE => "桂",
        Kind::GI => "銀",
        Kind::KI => "金",
        Kind::KA => "角",
        Kind::HI => "飛",
        Kind::OU => "玉",
        Kind::TO => "と",
        Kind::NY => "成香",
        Kind::NK => "成桂",
        Kind::NG => "成銀",
        Kind::UM => "馬",
        Kind::RY => "龍",
    }
}

/// The character a board diagram gives `kind` (R-NOT-006).
///
/// This is the *board* spelling, in which a promoted piece is one character
/// (`杏` `圭` `全`). A move writes it out instead — [`move_word`], which is a
/// different table (R-KI2-005).
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

/// A square, spelled `７六` rather than by its internal index.
pub(crate) struct Coordinate(pub(crate) shogi_core::Square);

impl fmt::Display for Coordinate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // A `Square` is 1-9 on both axes by construction, so neither lookup can
        // miss. They are still written as lookups rather than as indexing: this
        // is a `Display`, and a panic here would come out of whatever printed
        // the error rather than out of the code that built it.
        let file = self
            .0
            .file()
            .checked_sub(1)
            .and_then(|i| SANYOU_SUJI.get(i as usize));
        let rank = self
            .0
            .rank()
            .checked_sub(1)
            .and_then(|i| KANSUJI.get(i as usize));
        match (file, rank) {
            (Some(file), Some(rank)) => write!(f, "{file}{rank}"),
            _ => write!(f, "({}, {})", self.0.file(), self.0.rank()),
        }
    }
}

/// A list of squares, `７七 ８八`, for an origin that could be either.
pub(crate) struct Coordinates<'a>(pub(crate) &'a [shogi_core::Square]);

impl fmt::Display for Coordinates<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // The list is empty when no candidate matched the suffix the record
        // gave, which is a different fault from "two pieces could have done
        // this" and must not come out as a message that trails off.
        if self.0.is_empty() {
            return f.write_str("(no candidate)");
        }
        for (i, square) in self.0.iter().enumerate() {
            if i > 0 {
                f.write_str(" ")?;
            }
            write!(f, "{}", Coordinate(*square))?;
        }
        Ok(())
    }
}

/// A move, spelled from and to rather than by two internal indices.
///
/// A [`shogi_core::Move::Normal`] does not carry the piece, so it is written as
/// the two squares — `７七→７六` — rather than in the notation a kifu uses.
/// A drop does carry it, and `７六歩打` is what a file would say.
pub(crate) struct MoveText(pub(crate) shogi_core::Move);

impl fmt::Display for MoveText {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.0 {
            shogi_core::Move::Normal { from, to, promote } => {
                write!(f, "{}→{}", Coordinate(from), Coordinate(to))?;
                if promote {
                    f.write_str("成")?;
                }
                Ok(())
            }
            shogi_core::Move::Drop { piece, to } => {
                write!(
                    f,
                    "{}{}打",
                    Coordinate(to),
                    board_word(pk2k(piece.piece_kind()))
                )
            }
        }
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

#[cfg(test)]
mod tests {
    use super::*;
    use shogi_core::{Move, Piece, PieceKind, Square};

    // `Square`'s own `Debug` prints the internal index — `Square(61)` for 8八 —
    // which is neither a file nor a rank and cannot be found in the file the
    // reader was handed. Every error that names a square has to spell it.
    #[test]
    fn a_square_is_spelled_the_way_the_notation_writes_it() {
        let square = |file, rank| Square::new(file, rank).expect("a square");
        assert_eq!("８八", Coordinate(square(8, 8)).to_string());
        assert_eq!("１一", Coordinate(square(1, 1)).to_string());
        assert_eq!("９九", Coordinate(square(9, 9)).to_string());
    }

    #[test]
    fn a_move_names_both_of_its_squares() {
        let square = |file, rank| Square::new(file, rank).expect("a square");
        assert_eq!(
            "７七→７六",
            MoveText(Move::Normal {
                from: square(7, 7),
                to: square(7, 6),
                promote: false,
            })
            .to_string()
        );
        assert_eq!(
            "８八→２二成",
            MoveText(Move::Normal {
                from: square(8, 8),
                to: square(2, 2),
                promote: true,
            })
            .to_string()
        );
        assert_eq!(
            "５五歩打",
            MoveText(Move::Drop {
                piece: Piece::new(PieceKind::Pawn, shogi_core::Color::Black),
                to: square(5, 5),
            })
            .to_string()
        );
    }

    // An origin nothing could have come from is a different fault from an
    // origin two pieces could have come from. Printing the empty list as
    // nothing at all makes the two read the same.
    #[test]
    fn a_list_of_origins_says_so_when_it_is_empty() {
        let square = |file, rank| Square::new(file, rank).expect("a square");
        assert_eq!("(no candidate)", Coordinates(&[]).to_string());
        assert_eq!(
            "６八 ４九",
            Coordinates(&[square(6, 8), square(4, 9)]).to_string()
        );
    }
}
