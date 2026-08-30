//! Converters for [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
//!
//! Also provides implementation [`shogi_core::ToUsi`] for [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)

mod csa;
mod kakinoki;
mod ki2;
mod kif;

pub use self::csa::ToCsa;
pub use self::ki2::ToKi2;
pub use self::kif::ToKif;
use crate::jkf::JsonKifuFormat;
use shogi_core::{PartialPosition, Position, ToUsi};

impl ToUsi for JsonKifuFormat {
    /// # Errors
    ///
    /// Returns `Err` if `sink` fails, or if the record cannot be replayed into
    /// a position — a kifu that records an illegal move is valid input
    /// (R-RULE-002), and this trait has no way to say more than "failed".
    fn to_usi<W: std::fmt::Write>(&self, sink: &mut W) -> std::fmt::Result {
        let pos = Position::try_from(self).map_err(|_| std::fmt::Error)?;
        if pos.initial_position() == &PartialPosition::startpos() {
            sink.write_str("startpos")?;
        } else {
            sink.write_str("sfen ")?;
            pos.initial_position().to_sfen(sink)?;
        }
        if !pos.moves().is_empty() {
            sink.write_str(" moves")?;
            for mv in pos.moves() {
                sink.write_str(" ")?;
                mv.to_usi(sink)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::{ToCsa, ToKi2, ToKif};
    use crate::jkf::MoveFormat;

    /// `moves` is a plain `Vec`, so an empty one deserialises fine and reaches
    /// every writer. Index 0 is only a convention. A comment-only node is
    /// likewise valid JKF — neither `move` nor `special` is required.
    #[test]
    fn degenerate_records_do_not_panic() {
        for jkf in [
            JsonKifuFormat {
                moves: vec![],
                ..Default::default()
            },
            JsonKifuFormat {
                moves: vec![
                    MoveFormat::default(),
                    MoveFormat {
                        comments: Some(vec!["memo".to_owned()]),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            },
        ] {
            let _ = jkf.to_kif_owned();
            let _ = jkf.to_ki2_owned();
            let _ = jkf.to_csa_owned();
            let mut usi = String::new();
            let _ = jkf.to_usi(&mut usi);
        }
    }

    /// An unreplayable record is an error, not a panic: the trait returns
    /// `fmt::Result` and callers expect to be able to match on it.
    #[test]
    fn to_usi_reports_an_unreplayable_record() {
        let jkf = crate::parser::parse_jkf_str(
            r#"{"header":{},"initial":null,"moves":[{},{"move":{"color":0,"from":{"x":9,"y":9},"to":{"x":1,"y":1},"piece":"KY"}}]}"#,
        );
        // Either the parse rejects it or the writer does, but nothing unwinds.
        if let Ok(jkf) = jkf {
            let mut usi = String::new();
            let _ = jkf.to_usi(&mut usi);
        }
    }
}
