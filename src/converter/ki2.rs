use super::kakinoki::{write_header, write_initial, write_kansuji, write_sanyou_suji};
use crate::jkf::*;
use std::fmt::{Result, Write};

/// A type that is convertible to KI2 format.
pub trait ToKi2 {
    /// Write `self` in KI2 format.
    ///
    /// This function returns Err(core::fmt::Error)
    /// if and only if it fails to write to `sink`.
    fn to_ki2<W: Write>(&self, sink: &mut W) -> Result;

    /// Returns `self`'s string representation.
    fn to_ki2_owned(&self) -> String {
        let mut s = String::new();
        // guaranteed to be Ok(())
        let result = self.to_ki2(&mut s);
        debug_assert_eq!(result, Ok(()));
        s
    }
}

fn write_move_kind<W: Write>(kind: Kind, sink: &mut W) -> Result {
    match kind {
        Kind::FU => sink.write_str("歩"),
        Kind::KY => sink.write_str("香"),
        Kind::KE => sink.write_str("桂"),
        Kind::GI => sink.write_str("銀"),
        Kind::KI => sink.write_str("金"),
        Kind::KA => sink.write_str("角"),
        Kind::HI => sink.write_str("飛"),
        Kind::OU => sink.write_str("玉"),
        Kind::TO => sink.write_str("と"),
        Kind::NY => sink.write_str("成香"),
        Kind::NK => sink.write_str("成桂"),
        Kind::NG => sink.write_str("成銀"),
        Kind::UM => sink.write_str("馬"),
        Kind::RY => sink.write_str("竜"),
    }
}

/// Writes the KI2 notation for `moves`, deriving the disambiguating suffix from
/// `position` rather than from [`MoveMoveFormat::relative`].
///
/// KI2 carries no move origin, so a move that needs `左`/`右`/… and does not get
/// it cannot be read back. `relative` is derived data — a value the position
/// already determines — and trusting the field means any caller that skipped
/// [`JsonKifuFormat::populate_relative`] silently writes an unreadable file.
///
/// `position` becomes `None` once a move cannot be applied. Kifu recording an
/// illegal move are valid input (R-RULE-002), so the remaining moves fall back
/// to whatever `relative` holds instead of refusing to write.
fn write_moves<W: Write>(
    moves: &[MoveFormat],
    mut position: Option<shogi_core::PartialPosition>,
    sink: &mut W,
) -> Result {
    if let Some(comments) = &moves[0].comments {
        for comment in comments {
            if !comment.starts_with('&') {
                sink.write_char('*')?;
            }
            sink.write_str(comment)?;
            sink.write_char('\n')?;
        }
    }
    let mut it = moves[1..].iter().peekable();
    while let Some(mf) = it.next() {
        if let Some(mv) = &mf.move_ {
            match mv.color {
                Color::Black => sink.write_char('▲')?,
                Color::White => sink.write_char('△')?,
            }
            if mv.same.is_some() {
                sink.write_str("同")?;
            } else {
                write_sanyou_suji(mv.to.x, sink)?;
                write_kansuji(mv.to.y, sink)?;
            }
            write_move_kind(mv.piece, sink)?;
            let core_move = shogi_core::Move::try_from(mv).ok();
            let relative = match (&position, core_move) {
                (Some(pos), Some(core_move)) => {
                    crate::normalizer::infer_relative_from_position(pos, core_move)
                }
                _ => mv.relative,
            };
            if let Some(relative) = relative {
                match relative {
                    Relative::L => sink.write_str("左")?,
                    Relative::C => sink.write_str("直")?,
                    Relative::R => sink.write_str("右")?,
                    Relative::U => sink.write_str("上")?,
                    Relative::M => sink.write_str("寄")?,
                    Relative::D => sink.write_str("引")?,
                    Relative::LU => sink.write_str("左上")?,
                    Relative::LM => sink.write_str("左寄")?,
                    Relative::LD => sink.write_str("左引")?,
                    Relative::RU => sink.write_str("右上")?,
                    Relative::RM => sink.write_str("右寄")?,
                    Relative::RD => sink.write_str("右引")?,
                    Relative::H => sink.write_str("打")?,
                }
            }
            if let Some(promote) = mv.promote {
                if promote {
                    sink.write_str("成")?;
                } else {
                    sink.write_str("不成")?;
                }
            }
            position = position.and_then(|mut pos| {
                let core_move = core_move?;
                pos.make_move(core_move)?;
                Some(pos)
            });
        }
        if let Some(comments) = &mf.comments {
            sink.write_char('\n')?;
            for comment in comments {
                if !comment.starts_with('&') {
                    sink.write_char('*')?;
                }
                sink.write_str(comment)?;
                sink.write_char('\n')?;
            }
        } else if it.peek().is_some() {
            sink.write_char(' ')?;
        }
    }
    sink.write_char('\n')?;
    Ok(())
}

impl ToKi2 for JsonKifuFormat {
    fn to_ki2<W: Write>(&self, sink: &mut W) -> Result {
        write_header(&self.header, sink)?;
        write_initial(&self.initial, true, sink)?;
        write_moves(&self.moves, self.starting_position(), sink)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The disambiguating suffix comes from the position, not from `relative`,
    // so a value that never went through `populate_relative` still produces KI2
    // that can be read back. Two black bishops on 7a and 3a both reach 5c.
    #[test]
    fn disambiguation_does_not_depend_on_relative_field() {
        let src = "\
手合割：その他
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
   1 ５三角成(71)   ( 0:00/00:00:00)
";
        let mut jkf = crate::parser::parse_kif_str(src).expect("parses");
        assert_eq!(
            None,
            jkf.moves[1].move_.expect("a move").relative,
            "the KIF path leaves `relative` empty; the point is that KI2 still works"
        );
        assert!(
            jkf.to_ki2_owned().contains("▲５三角左成"),
            "expected a disambiguated move, got {:?}",
            jkf.to_ki2_owned()
        );
        // The field being filled in must not change the answer.
        jkf.populate_relative().expect("populates");
        assert!(jkf.to_ki2_owned().contains("▲５三角左成"));
    }

    #[test]
    fn to_ki2_default() {
        assert_eq!("\n", JsonKifuFormat::default().to_ki2_owned());
    }

    #[test]
    fn to_ki2_moves() {
        assert_eq!(
            "▲２六歩 △８四歩 ▲２五歩\n",
            JsonKifuFormat {
                moves: vec![
                    MoveFormat::default(),
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(PlaceFormat { x: 2, y: 7 }),
                            to: PlaceFormat { x: 2, y: 6 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        ..Default::default()
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::White,
                            from: Some(PlaceFormat { x: 8, y: 3 }),
                            to: PlaceFormat { x: 8, y: 4 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        ..Default::default()
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(PlaceFormat { x: 2, y: 6 }),
                            to: PlaceFormat { x: 2, y: 5 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        ..Default::default()
                    }
                ],
                ..Default::default()
            }
            .to_ki2_owned()
        );
    }
}
