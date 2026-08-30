use crate::error::ParseError;
use crate::jkf::*;
use crate::normalizer::HIRATE_BOARD;
use csa::{GameRecord, Position};
use std::collections::HashMap;
use std::time::Duration;

impl TryFrom<GameRecord> for JsonKifuFormat {
    type Error = ParseError;

    fn try_from(record: GameRecord) -> Result<Self, Self::Error> {
        // Header
        let mut header = HashMap::new();
        if let Some(s) = record.black_player {
            header.insert(String::from("先手"), s);
        }
        if let Some(s) = record.white_player {
            header.insert(String::from("後手"), s);
        }
        if let Some(s) = record.event {
            header.insert(String::from("棋戦"), s);
        }
        if let Some(s) = record.site {
            header.insert(String::from("場所"), s);
        }
        if let Some(t) = record.start_time {
            header.insert(String::from("開始日時"), t.to_string());
        }
        if let Some(t) = record.end_time {
            header.insert(String::from("終了日時"), t.to_string());
        }
        if let Some(t) = record.time_limit {
            header.insert(String::from("持ち時間"), t.to_string());
        }
        if let Some(s) = record.opening {
            header.insert(String::from("戦型"), s);
        }
        // Initial
        let initial = Some(Initial::try_from(record.start_pos)?);
        // Moves
        let mut moves = vec![MoveFormat::default()];
        for m in record.moves {
            moves.push(m.try_into()?);
        }
        Ok(Self {
            header,
            initial,
            moves,
        })
    }
}

/// A square inside the board, as a `(file, rank)` index pair.
///
/// The `csa` crate accepts any two digits, so `P+09FU` and `PI00FU` reach here
/// with a zero. Positions are 1-9 in each direction (R-CSA-005); anything else
/// is a broken file, not a square to index with.
fn board_index(sq: csa::Square) -> Result<(usize, usize), ParseError> {
    if (1..=9).contains(&sq.file) && (1..=9).contains(&sq.rank) {
        Ok((sq.file as usize - 1, sq.rank as usize - 1))
    } else {
        Err(ParseError::CsaConvert("position out of range"))
    }
}

impl TryFrom<Position> for Initial {
    type Error = ParseError;

    fn try_from(mut pos: Position) -> Result<Self, Self::Error> {
        let mut all_pieces = Hand {
            FU: 18,
            KY: 4,
            KE: 4,
            GI: 4,
            KI: 4,
            KA: 2,
            HI: 2,
        };
        // split to hands' and board's
        let mut hand_pieces = Vec::new();
        pos.add_pieces.retain(|&(c, sq, pt)| {
            if sq.file == 0 && sq.rank == 0 {
                hand_pieces.push((c, pt));
                false
            } else {
                true
            }
        });
        // Color
        let color = pos.side_to_move.into();
        // Board
        let board = if let Some(grid) = pos.bulk {
            // 一括表現
            let mut b = [[Piece::empty(); 9]; 9];
            for (i, row) in grid.iter().enumerate() {
                for (j, &col) in row.iter().enumerate() {
                    b[8 - j][i] = if let Some((c, pt)) = col {
                        Piece::from((c, pt))
                    } else {
                        Piece::empty()
                    };
                }
            }
            b
        } else if pos.add_pieces.is_empty() {
            // 平手初期配置と駒落ち
            let mut b = HIRATE_BOARD;
            for &(sq, _) in &pos.drop_pieces {
                let (file, rank) = board_index(sq)?;
                b[file][rank] = Piece::empty()
            }
            b
        } else {
            // 駒別単独表現
            let mut b = [[Piece::empty(); 9]; 9];
            for &(c, sq, pt) in &pos.add_pieces {
                let (file, rank) = board_index(sq)?;
                b[file][rank] = Piece::from((c, pt));
            }
            b
        };
        for row in &board {
            for col in row {
                if let Some(unpromoted) = col.kind.map(Kind::unpromoted) {
                    if unpromoted != Kind::OU && all_pieces.decrement(unpromoted).is_none() {
                        // More of that piece on the board than a set holds.
                        return Err(ParseError::CsaConvert("too many pieces on the board"));
                    }
                }
            }
        }
        // Hands
        let mut hands = [Hand::default(); 2];
        for &(c, pt) in &hand_pieces {
            let index = Into::<Color>::into(c) as usize;
            match pt.try_into() {
                Ok(kind) => {
                    if hands[index].increment(kind).is_none() {
                        // A king or a promoted piece in hand (R-CSA-006).
                        return Err(ParseError::CsaConvert("piece cannot be held in hand"));
                    }
                }
                // `AL`: everything still unaccounted for.
                Err(_) => hands[index] = all_pieces,
            }
        }
        Ok(Self {
            preset: Preset::PresetOther,
            data: Some(StateFormat {
                color,
                board,
                hands,
            }),
        })
    }
}

impl TryFrom<csa::MoveRecord> for MoveFormat {
    type Error = ParseError;

    fn try_from(m: csa::MoveRecord) -> Result<Self, Self::Error> {
        let time = m.time.map(|d| Time {
            now: d.into(),
            total: TimeFormat::default(),
        });
        match m.action {
            csa::Action::Move(c, from, to, pt) => Ok(MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: c.into(),
                    // CSA spells a drop with `00` for the origin (R-CSA-007);
                    // JKF says it by leaving `from` out (R-JKF-003).
                    from: Some(PlaceFormat::from(from))
                        .filter(|p| *p != crate::normalizer::ORIGIN_UNSTATED),
                    to: to.into(),
                    piece: pt.try_into()?,
                    same: None,
                    promote: None,
                    capture: None,
                    relative: None,
                }),
                time,
                ..Default::default()
            }),
            csa::Action::Toryo => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialToryo),
                ..Default::default()
            }),
            csa::Action::Chudan => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialChudan),
                ..Default::default()
            }),
            csa::Action::Sennichite => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialSennichite),
                ..Default::default()
            }),
            csa::Action::TimeUp => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialTimeUp),
                ..Default::default()
            }),
            csa::Action::IllegalMove => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialIllegalMove),
                ..Default::default()
            }),
            csa::Action::IllegalAction(csa::Color::Black) => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialIllegalActionBlack),
                ..Default::default()
            }),
            csa::Action::IllegalAction(csa::Color::White) => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialIllegalActionWhite),
                ..Default::default()
            }),
            csa::Action::Jishogi => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialJishogi),
                ..Default::default()
            }),
            csa::Action::Kachi => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialKachi),
                ..Default::default()
            }),
            csa::Action::Hikiwake => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialHikiwake),
                ..Default::default()
            }),
            csa::Action::Matta => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialMatta),
                ..Default::default()
            }),
            csa::Action::Tsumi => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialTsumi),
                ..Default::default()
            }),
            csa::Action::Fuzumi => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialFuzumi),
                ..Default::default()
            }),
            csa::Action::Error => Ok(MoveFormat {
                time,
                special: Some(MoveSpecial::SpecialError),
                ..Default::default()
            }),
        }
    }
}

impl From<(csa::Color, csa::PieceType)> for Piece {
    fn from((c, pt): (csa::Color, csa::PieceType)) -> Self {
        Piece {
            color: Some(c.into()),
            kind: pt.try_into().ok(),
        }
    }
}

impl From<csa::Color> for Color {
    fn from(c: csa::Color) -> Self {
        match c {
            csa::Color::Black => Color::Black,
            csa::Color::White => Color::White,
        }
    }
}

impl From<Duration> for TimeFormat {
    fn from(d: Duration) -> Self {
        let s = d.as_secs();
        let m = (s / 60) % 60;
        let h = s / 3600;
        TimeFormat {
            h: if h > 0 { Some(h as u8) } else { None },
            m: m as u8,
            s: (s % 60) as u8,
        }
    }
}

impl From<csa::Square> for PlaceFormat {
    fn from(sq: csa::Square) -> Self {
        PlaceFormat {
            x: sq.file,
            y: sq.rank,
        }
    }
}

impl TryFrom<csa::PieceType> for Kind {
    type Error = ParseError;

    fn try_from(pt: csa::PieceType) -> Result<Self, Self::Error> {
        match pt {
            csa::PieceType::Pawn => Ok(Kind::FU),
            csa::PieceType::Lance => Ok(Kind::KY),
            csa::PieceType::Knight => Ok(Kind::KE),
            csa::PieceType::Silver => Ok(Kind::GI),
            csa::PieceType::Gold => Ok(Kind::KI),
            csa::PieceType::Bishop => Ok(Kind::KA),
            csa::PieceType::Rook => Ok(Kind::HI),
            csa::PieceType::King => Ok(Kind::OU),
            csa::PieceType::ProPawn => Ok(Kind::TO),
            csa::PieceType::ProLance => Ok(Kind::NY),
            csa::PieceType::ProKnight => Ok(Kind::NK),
            csa::PieceType::ProSilver => Ok(Kind::NG),
            csa::PieceType::Horse => Ok(Kind::UM),
            csa::PieceType::Dragon => Ok(Kind::RY),
            csa::PieceType::All => {
                Err(ParseError::CsaConvert("`AL` cannot be converted to `Kind`"))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::parser::parse_csa_str;

    /// The `csa` crate takes any two digits as a position and any piece type in
    /// hand, so a broken file reaches the conversion with a zero file or a king
    /// on the stand. Those have to come back as errors: the consumer is a Tauri
    /// command, and a panic there takes the application with it (R-REQ-004).
    #[test]
    fn broken_positions_are_errors() {
        for src in [
            // A king cannot be held in hand (R-CSA-006).
            "V2.2\nPI\nP+00OU\n+\n",
            // Positions are 1-9 in each direction (R-CSA-005).
            "V2.2\nP+05FU\n+\n",
            "V2.2\nP+50FU\n+\n",
            "V2.2\nPI00FU\n+\n",
        ] {
            assert!(parse_csa_str(src).is_err(), "expected an error for {src:?}");
        }
    }
}
