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

impl JsonKifuFormat {
    /// Returns `self` in USI format, or the error [`ToUsi::to_usi`] gave.
    ///
    /// # Errors
    ///
    /// Returns `Err` if the record cannot be replayed into a position. A kifu
    /// recording an illegal move is valid input (R-RULE-002), so this is a
    /// value a file can produce.
    ///
    /// Use this rather than [`ToUsi::to_usi_owned`]. That one is a default
    /// method in `shogi_core` which asserts the write succeeded: with
    /// `debug_assertions` it panics, and without them it hands back an empty
    /// string. The consumer is a Tauri command, so the first is a crash and the
    /// second writes an empty `.usi` file over a real one.
    pub fn try_to_usi_owned(&self) -> std::result::Result<String, std::fmt::Error> {
        let mut s = String::new();
        self.to_usi(&mut s)?;
        Ok(s)
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

    /// A record that cannot be replayed is an error, not a panic and not an
    /// empty string. `ToUsi::to_usi_owned` gives one or the other because of the
    /// `debug_assert` in its default body, so this crate offers its own.
    ///
    /// The move below starts from an empty square, so replaying it fails. It
    /// goes in directly rather than through a parser, because normalizing would
    /// reject it first.
    #[test]
    fn an_unreplayable_record_is_an_error() {
        use crate::jkf::{Color, Initial, Kind, MoveMoveFormat, PlaceFormat, Preset};
        let jkf = JsonKifuFormat {
            initial: Some(Initial {
                preset: Preset::PresetHirate,
                data: None,
            }),
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 5, y: 5 }),
                        to: PlaceFormat { x: 5, y: 4 },
                        piece: Kind::FU,
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
        assert_eq!(Err(std::fmt::Error), jkf.try_to_usi_owned());
    }
}
