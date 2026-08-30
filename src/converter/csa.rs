use crate::jkf::*;
use std::collections::HashMap;
use std::fmt::{Result, Write};

/// A type that is convertible to CSA format.
pub trait ToCsa {
    /// Write `self` in CSA format.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `sink` fails, or if the record holds something CSA
    /// cannot spell — a coordinate outside the board, or more pieces in hand
    /// than a set contains. Such a record comes from outside this crate; it is
    /// not a value any parser here produces.
    fn to_csa<W: Write>(&self, sink: &mut W) -> Result;

    /// Returns `self`'s string representation, or the error [`Self::to_csa`]
    /// gave.
    fn try_to_csa_owned(&self) -> std::result::Result<String, std::fmt::Error> {
        let mut s = String::new();
        self.to_csa(&mut s)?;
        Ok(s)
    }

    /// Returns `self`'s string representation.
    ///
    /// A record that cannot be spelled in CSA yields whatever was written
    /// before the failure. Use [`Self::try_to_csa_owned`] to see the error
    /// instead of a truncated file.
    #[deprecated(
        note = "returns a truncated string on failure, which the caller writes to disk as if it were the whole record. Use try_to_csa_owned."
    )]
    fn to_csa_owned(&self) -> String {
        let mut s = String::new();
        let _ = self.to_csa(&mut s);
        s
    }
}

impl ToCsa for JsonKifuFormat {
    fn to_csa<W: Write>(&self, sink: &mut W) -> Result {
        write_header(&self.header, sink)?;
        write_initial(&self.initial, sink)?;
        // Index 0 holds the initial position's comments, not a ply.
        write_moves(self.moves.get(1..).unwrap_or_default(), sink)?;
        Ok(())
    }
}

fn write_color<W: Write>(c: Color, sink: &mut W) -> Result {
    match c {
        Color::Black => sink.write_char('+')?,
        Color::White => sink.write_char('-')?,
    }
    Ok(())
}

fn write_kind<W: Write>(kind: Kind, sink: &mut W) -> Result {
    match kind {
        Kind::FU => sink.write_str("FU")?,
        Kind::KY => sink.write_str("KY")?,
        Kind::KE => sink.write_str("KE")?,
        Kind::GI => sink.write_str("GI")?,
        Kind::KI => sink.write_str("KI")?,
        Kind::KA => sink.write_str("KA")?,
        Kind::HI => sink.write_str("HI")?,
        Kind::OU => sink.write_str("OU")?,
        Kind::TO => sink.write_str("TO")?,
        Kind::NY => sink.write_str("NY")?,
        Kind::NK => sink.write_str("NK")?,
        Kind::NG => sink.write_str("NG")?,
        Kind::UM => sink.write_str("UM")?,
        Kind::RY => sink.write_str("RY")?,
    }
    Ok(())
}

/// Writes a square, or `00` for the hand a drop comes from (R-CSA-007).
///
/// A `Some` that is not a square on the board would be spelled `00` by the same
/// digits and read back as a drop, turning a move into one — so it is an error,
/// not a coordinate.
fn write_place<W: Write>(place: &Option<PlaceFormat>, sink: &mut W) -> Result {
    if let Some(p) = place {
        shogi_core::Square::try_from(p).map_err(|_| std::fmt::Error)?;
        sink.write_fmt(format_args!("{}{}", p.x, p.y))?;
    } else {
        sink.write_str("00")?;
    }
    Ok(())
}

fn write_header<W: Write>(header: &HashMap<String, String>, sink: &mut W) -> Result {
    sink.write_str("V2.2\n")?;
    if let Some(s) = header.get("先手").or_else(|| header.get("下手")) {
        if !s.is_empty() {
            sink.write_fmt(format_args!("N+{}\n", s))?;
        }
    }
    if let Some(s) = header.get("後手").or_else(|| header.get("上手")) {
        if !s.is_empty() {
            sink.write_fmt(format_args!("N-{}\n", s))?;
        }
    }
    if let Some(s) = header.get("棋戦") {
        sink.write_fmt(format_args!("$EVENT:{}\n", s))?;
    }
    if let Some(s) = header.get("場所") {
        sink.write_fmt(format_args!("$SITE:{}\n", s))?;
    }
    if let Some(s) = header.get("戦型") {
        sink.write_fmt(format_args!("$OPENING:{}\n", s))?;
    }
    Ok(())
}

fn write_initial_data<W: Write>(data: &StateFormat, sink: &mut W) -> Result {
    for i in 0..9 {
        sink.write_fmt(format_args!("P{}", i + 1))?;
        for j in 0..9 {
            let p = data.board[8 - j][i];
            if let (Some(c), Some(kind)) = (p.color, p.kind) {
                write_color(c, sink)?;
                write_kind(kind, sink)?;
            } else {
                sink.write_str(" * ")?;
            }
        }
        sink.write_char('\n')?;
    }
    for (i, hand) in data.hands.iter().enumerate() {
        if hand == &Hand::default() {
            continue;
        }
        // R-CSA-006: `AL` gives the rest of the pieces to one side in a single
        // line. It is read but never written — spelling every piece out is
        // always valid and does not depend on what "the rest" means here.
        if i == 0 {
            sink.write_str("P+")?;
        } else {
            sink.write_str("P-")?;
        }
        (0..hand.HI).try_for_each(|_| sink.write_str("00HI"))?;
        (0..hand.KA).try_for_each(|_| sink.write_str("00KA"))?;
        (0..hand.KI).try_for_each(|_| sink.write_str("00KI"))?;
        (0..hand.GI).try_for_each(|_| sink.write_str("00GI"))?;
        (0..hand.KE).try_for_each(|_| sink.write_str("00KE"))?;
        (0..hand.KY).try_for_each(|_| sink.write_str("00KY"))?;
        (0..hand.FU).try_for_each(|_| sink.write_str("00FU"))?;
        sink.write_char('\n')?;
    }
    write_color(data.color, sink)?;
    Ok(())
}

fn write_initial_preset<W: Write>(preset: Preset, sink: &mut W) -> Result {
    // `PI` is the even game; each removed piece follows as position + kind
    // (R-CSA-006). `その他` never reaches here — it carries a board instead.
    let Some(handicap) = crate::handicap::lookup(preset) else {
        return Err(std::fmt::Error);
    };
    sink.write_str("PI")?;
    for &(file, rank, kind) in handicap.removed {
        sink.write_fmt(format_args!("{file}{rank}"))?;
        write_kind(kind, sink)?;
    }
    sink.write_char('\n')?;
    write_color(crate::handicap::side_to_move(preset), sink)?;
    Ok(())
}

fn write_initial<W: Write>(initial: &Option<Initial>, sink: &mut W) -> Result {
    if let Some(initial) = initial {
        if let Some(data) = &initial.data {
            write_initial_data(data, sink)?;
        } else {
            write_initial_preset(initial.preset, sink)?;
        }
    } else {
        sink.write_str("PI\n+")?;
    }
    sink.write_char('\n')?;
    Ok(())
}

fn write_moves<W: Write>(moves: &[MoveFormat], sink: &mut W) -> Result {
    for mf in moves {
        // A node with neither a move nor an outcome carries only comments,
        // which JKF allows. It gets no move line, and therefore no `T` line
        // either: a reader takes `T` as the time spent on the move above it
        // (R-CSA-007), so writing one here overwrites the previous move's time.
        let has_line = mf.move_.is_some() || mf.special.is_some();
        if let Some(mv) = mf.move_ {
            write_color(mv.color, sink)?;
            write_place(&mv.from, sink)?;
            write_place(&Some(mv.to), sink)?;
            let kind = if mv.promote.unwrap_or_default() {
                mv.piece.promoted()
            } else {
                mv.piece
            };
            write_kind(kind, sink)?;
            sink.write_str("\n")?;
        } else if let Some(special) = &mf.special {
            sink.write_char('%')?;
            sink.write_str(special.csa_word())?;
            sink.write_str("\n")?;
        }
        if let Some(time) = mf.time.filter(|_| has_line) {
            let sec = time.now.h.unwrap_or_default() as u64 * 3600
                + time.now.m as u64 * 60
                + time.now.s as u64;
            sink.write_fmt(format_args!("T{}\n", sec))?;
        }
        if let Some(comments) = &mf.comments {
            for comment in comments {
                sink.write_fmt(format_args!("'{}\n", comment))?;
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn to_csa_default() {
        assert_eq!(
            r#"
V2.2
PI
+
"#[1..],
            JsonKifuFormat::default()
                .try_to_csa_owned()
                .expect("writes CSA")
        );
    }
}
