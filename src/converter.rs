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
            let _ = jkf.try_to_kif_owned();
            let _ = jkf.try_to_ki2_owned();
            let _ = jkf.try_to_csa_owned();
            let mut usi = String::new();
            let _ = jkf.to_usi(&mut usi);
        }
    }

    /// CSA reads `T<sec>` as the time spent on the move above it (R-CSA-007),
    /// so a node that writes no move line must not write one either — the time
    /// would silently replace the previous move's.
    #[test]
    fn a_node_without_a_move_line_writes_no_time_line() {
        use crate::jkf::{Color, Kind, MoveMoveFormat, PlaceFormat, Time, TimeFormat};
        let ticks = |s: u8| Time {
            now: TimeFormat { h: None, m: 0, s },
            total: TimeFormat { h: None, m: 0, s },
        };
        let jkf = JsonKifuFormat {
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 7, y: 7 }),
                        to: PlaceFormat { x: 7, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    time: Some(ticks(30)),
                    ..Default::default()
                },
                MoveFormat {
                    comments: Some(vec!["memo".to_owned()]),
                    time: Some(ticks(99)),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        let csa = jkf.try_to_csa_owned().expect("writes CSA");
        assert_eq!(
            vec!["T30"],
            csa.lines()
                .filter(|l| l.starts_with('T'))
                .collect::<Vec<_>>(),
            "only the move gets a time line: {csa:?}"
        );
    }

    /// One row of the outcome vocabulary. A struct rather than a tuple because
    /// six of the seven fields are the same two types and a swapped pair would
    /// still compile.
    struct Row {
        special: crate::jkf::MoveSpecial,
        kif_word: &'static str,
        csa_word: &'static str,
        ki2_phrase: &'static str,
        after_kif: crate::jkf::MoveSpecial,
        after_ki2: crate::jkf::MoveSpecial,
        after_csa: Option<crate::jkf::MoveSpecial>,
    }

    /// Every `MoveSpecial` against the word each format writes for it and what
    /// survives a round trip through that format.
    ///
    /// Transcribed from `research/tables/40-vocabulary.md` 表2 — KIF words from
    /// R-KIF-007, CSA keywords from R-CSA-007, KI2 phrases from D5. **Nothing
    /// here was read off a run of the code**; where the three formats lose
    /// information the row says so, because the difference between a known
    /// limit of a format and a writer quietly changing what a record says is
    /// the whole point. Pinning `to_kif` to `中断` for every outcome has to fail
    /// here, and so does pinning `to_csa` to `%CHUDAN`.
    #[test]
    fn every_outcome_has_the_word_each_format_writes() {
        use crate::jkf::MoveSpecial::*;
        use crate::jkf::{Color, Initial, Kind, MoveMoveFormat, PlaceFormat, Preset};

        // Two moves, so the outcome sits at ply 3 with Black to move.
        const TABLE: [Row; 14] = [
            Row {
                special: SpecialToryo,
                kif_word: "投了",
                csa_word: "%TORYO",
                ki2_phrase: "後手の勝ち",
                after_kif: SpecialToryo,
                after_ki2: SpecialToryo,
                after_csa: Some(SpecialToryo),
            },
            Row {
                special: SpecialChudan,
                kif_word: "中断",
                csa_word: "%CHUDAN",
                ki2_phrase: "中断",
                after_kif: SpecialChudan,
                after_ki2: SpecialChudan,
                after_csa: Some(SpecialChudan),
            },
            Row {
                special: SpecialSennichite,
                kif_word: "千日手",
                csa_word: "%SENNICHITE",
                ki2_phrase: "千日手",
                after_kif: SpecialSennichite,
                after_ki2: SpecialSennichite,
                after_csa: Some(SpecialSennichite),
            },
            // GAP-012: the `csa` crate reads 10 of the 14 keywords. The four added
            // in CSA V2.1/V2.2 are written correctly and dropped on the way back
            // in, so the arms for them in `src/csa.rs` never run.
            Row {
                special: SpecialTimeUp,
                kif_word: "切れ負け",
                csa_word: "%TIME_UP",
                ki2_phrase: "時間切れにより後手の勝ち",
                after_kif: SpecialTimeUp,
                after_ki2: SpecialTimeUp,
                after_csa: None,
            },
            Row {
                special: SpecialIllegalMove,
                kif_word: "反則負け",
                csa_word: "%ILLEGAL_MOVE",
                ki2_phrase: "先手の反則負け",
                after_kif: SpecialIllegalMove,
                after_ki2: SpecialIllegalMove,
                after_csa: None,
            },
            // KIF's 反則勝ち is relative to whose turn it is, so it cannot say that
            // Black fouled while Black is to move. The KI2 phrase names the
            // winner outright and keeps the direction.
            Row {
                special: SpecialIllegalActionBlack,
                kif_word: "反則勝ち",
                csa_word: "%+ILLEGAL_ACTION",
                ki2_phrase: "後手の反則勝ち",
                after_kif: SpecialIllegalActionWhite,
                after_ki2: SpecialIllegalActionBlack,
                after_csa: None,
            },
            Row {
                special: SpecialIllegalActionWhite,
                kif_word: "反則勝ち",
                csa_word: "%-ILLEGAL_ACTION",
                ki2_phrase: "先手の反則勝ち",
                after_kif: SpecialIllegalActionWhite,
                after_ki2: SpecialIllegalActionWhite,
                after_csa: None,
            },
            Row {
                special: SpecialJishogi,
                kif_word: "持将棋",
                csa_word: "%JISHOGI",
                ki2_phrase: "持将棋",
                after_kif: SpecialJishogi,
                after_ki2: SpecialJishogi,
                after_csa: Some(SpecialJishogi),
            },
            Row {
                special: SpecialKachi,
                kif_word: "入玉勝ち",
                csa_word: "%KACHI",
                ki2_phrase: "先手の入玉勝ち",
                after_kif: SpecialKachi,
                after_ki2: SpecialKachi,
                after_csa: Some(SpecialKachi),
            },
            // R-KIF-007 has no word for a declared draw, and 持将棋 is the
            // closest thing KIF can say, so `%HIKIWAKE` comes back as
            // `%JISHOGI`. 中断 would lose that the game was drawn at all.
            Row {
                special: SpecialHikiwake,
                kif_word: "持将棋",
                csa_word: "%HIKIWAKE",
                ki2_phrase: "持将棋",
                after_kif: SpecialJishogi,
                after_ki2: SpecialJishogi,
                after_csa: Some(SpecialHikiwake),
            },
            // 待った and エラー have no KIF word at all (R-KIF-007), so they
            // collapse onto 中断: the game stopped here, and why is lost.
            Row {
                special: SpecialMatta,
                kif_word: "中断",
                csa_word: "%MATTA",
                ki2_phrase: "中断",
                after_kif: SpecialChudan,
                after_ki2: SpecialChudan,
                after_csa: Some(SpecialMatta),
            },
            Row {
                special: SpecialTsumi,
                kif_word: "詰み",
                csa_word: "%TSUMI",
                ki2_phrase: "詰み",
                after_kif: SpecialTsumi,
                after_ki2: SpecialTsumi,
                after_csa: Some(SpecialTsumi),
            },
            Row {
                special: SpecialFuzumi,
                kif_word: "不詰",
                csa_word: "%FUZUMI",
                ki2_phrase: "不詰",
                after_kif: SpecialFuzumi,
                after_ki2: SpecialFuzumi,
                after_csa: Some(SpecialFuzumi),
            },
            Row {
                special: SpecialError,
                kif_word: "中断",
                csa_word: "%ERROR",
                ki2_phrase: "中断",
                after_kif: SpecialChudan,
                after_ki2: SpecialChudan,
                after_csa: Some(SpecialError),
            },
        ];

        fn pawn(color: Color, fx: u8, fy: u8, tx: u8, ty: u8) -> MoveFormat {
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color,
                    from: Some(PlaceFormat { x: fx, y: fy }),
                    to: PlaceFormat { x: tx, y: ty },
                    piece: Kind::FU,
                    same: None,
                    promote: None,
                    capture: None,
                    relative: None,
                }),
                ..Default::default()
            }
        }

        fn last<E>(
            text: &str,
            read: fn(&str) -> Result<JsonKifuFormat, E>,
        ) -> Option<crate::jkf::MoveSpecial> {
            read(text).ok()?.moves.last()?.special
        }

        for row in TABLE {
            let special = row.special;
            let jkf = JsonKifuFormat {
                initial: Some(Initial {
                    preset: Preset::PresetHirate,
                    data: None,
                }),
                moves: vec![
                    MoveFormat::default(),
                    pawn(Color::Black, 7, 7, 7, 6),
                    pawn(Color::White, 3, 3, 3, 4),
                    MoveFormat {
                        special: Some(special),
                        ..Default::default()
                    },
                ],
                ..Default::default()
            };

            let kif = jkf.try_to_kif_owned().expect("writes KIF");
            assert!(
                kif.contains(&format!("   3 {}", row.kif_word)),
                "{special:?} in KIF: {kif:?}"
            );
            let csa = jkf.try_to_csa_owned().expect("writes CSA");
            assert!(
                csa.lines().any(|l| l == row.csa_word),
                "{special:?} in CSA: {csa:?}"
            );
            let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
            assert!(
                ki2.contains(&format!("まで2手で{}", row.ki2_phrase)),
                "{special:?} in KI2: {ki2:?}"
            );

            assert_eq!(
                Some(row.after_kif),
                last(&kif, crate::parser::parse_kif_str),
                "{special:?} through KIF"
            );
            assert_eq!(
                Some(row.after_ki2),
                last(&ki2, crate::parser::parse_ki2_str),
                "{special:?} through KI2"
            );
            assert_eq!(
                row.after_csa,
                last(&csa, crate::parser::parse_csa_str),
                "{special:?} through CSA"
            );
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
