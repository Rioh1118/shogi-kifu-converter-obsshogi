use super::kakinoki::{write_header, write_initial, write_kansuji, write_sanyou_suji};
use crate::jkf::*;
use std::fmt::{Result, Write};

/// A type that is convertible to KIF format.
pub trait ToKif {
    /// Write `self` in KIF format.
    ///
    /// # Errors
    ///
    /// Returns `Err` if `sink` fails, or if the record holds something KIF
    /// cannot spell — a coordinate outside the board, or more pieces in hand
    /// than a set contains. Such a record comes from outside this crate; it is
    /// not a value any parser here produces.
    fn to_kif<W: Write>(&self, sink: &mut W) -> Result;

    /// Returns `self`'s string representation, or the error [`Self::to_kif`]
    /// gave.
    fn try_to_kif_owned(&self) -> std::result::Result<String, std::fmt::Error> {
        let mut s = String::new();
        self.to_kif(&mut s)?;
        Ok(s)
    }

    /// Returns `self`'s string representation.
    ///
    /// A record that cannot be spelled in KIF yields whatever was written
    /// before the failure. Use [`Self::try_to_kif_owned`] to see the error
    /// instead of a truncated file.
    fn to_kif_owned(&self) -> String {
        let mut s = String::new();
        let _ = self.to_kif(&mut s);
        s
    }
}

impl ToKif for JsonKifuFormat {
    fn to_kif<W: Write>(&self, sink: &mut W) -> Result {
        write_header(&self.header, sink)?;
        write_initial(&self.initial, false, sink)?;
        write_moves(&self.moves, sink)?;
        Ok(())
    }
}

fn write_move_kind<W: Write>(kind: Kind, sink: &mut W, offset: &mut usize) -> Result {
    match kind {
        Kind::FU => sink.write_str("歩")?,
        Kind::KY => sink.write_str("香")?,
        Kind::KE => sink.write_str("桂")?,
        Kind::GI => sink.write_str("銀")?,
        Kind::KI => sink.write_str("金")?,
        Kind::KA => sink.write_str("角")?,
        Kind::HI => sink.write_str("飛")?,
        Kind::OU => sink.write_str("玉")?,
        Kind::TO => sink.write_str("と")?,
        Kind::NY => {
            sink.write_str("成香")?;
            *offset += 2;
        }
        Kind::NK => {
            sink.write_str("成桂")?;
            *offset += 2;
        }
        Kind::NG => {
            sink.write_str("成銀")?;
            *offset += 2;
        }
        Kind::UM => sink.write_str("馬")?,
        Kind::RY => sink.write_str("龍")?,
    }
    *offset += 2;
    Ok(())
}

fn write_move_lines<W: Write>(moves: &[MoveFormat], index: usize, sink: &mut W) -> Result {
    let mut forks_stack = Vec::new();
    // The ply is the number of the line being written, not the position in the
    // array. A node with neither a move nor an outcome carries only comments,
    // which JKF allows and KIF has no line for; counting it anyway leaves a gap
    // in the move numbers and shifts every `変化：N手` after it, so the branch
    // lands on the wrong move — or on nothing, and is dropped.
    let mut i = index;
    for mf in moves {
        let has_line = mf.move_.is_some() || mf.special.is_some();
        if has_line {
            sink.write_fmt(format_args!("{:4} ", i))?;
        }
        let mut offset = 0;
        if let Some(mv) = &mf.move_ {
            if mv.same.is_some() {
                sink.write_str("同　")?;
            } else {
                write_sanyou_suji(mv.to.x, sink)?;
                write_kansuji(mv.to.y, sink)?;
            }
            offset += 4;
            write_move_kind(mv.piece, sink, &mut offset)?;
            if mv.promote.unwrap_or_default() {
                sink.write_char('成')?;
                offset += 2;
            }
            if let Some(from) = mv.from {
                sink.write_fmt(format_args!("({}{})", from.x, from.y))?;
                offset += 4;
            } else {
                sink.write_char('打')?;
                offset += 2;
            }
        } else if let Some(special) = &mf.special {
            // 待った and エラー have no KIF word (R-KIF-007). 中断 is the
            // closest thing KIF can say — the game stopped here — so that is
            // what a reader will make of them, and the distinction is lost.
            let word = special.kif_word().unwrap_or("中断");
            sink.write_str(word)?;
            // The time column is measured in half-widths and every one of these
            // words is full-width.
            offset += word.chars().count() * 2;
        }
        if has_line {
            if let Some(time) = mf.time {
                (0..13usize.saturating_sub(offset)).try_for_each(|_| sink.write_char(' '))?;
                sink.write_fmt(format_args!(
                    "({:2}:{:02}/{:02}:{:02}:{:02})",
                    time.now.m,
                    time.now.s,
                    time.total.h.unwrap_or_default(),
                    time.total.m,
                    time.total.s
                ))?;
            }
            sink.write_char('\n')?;
        }
        if let Some(comments) = &mf.comments {
            for comment in comments {
                if !comment.starts_with('&') {
                    sink.write_char('*')?;
                }
                sink.write_str(comment)?;
                sink.write_char('\n')?;
            }
        }
        if let Some(ref forks) = mf.forks {
            for fork in forks {
                forks_stack.push((i, fork));
            }
        }
        if has_line {
            i += 1;
        }
    }
    while let Some((i, fork)) = forks_stack.pop() {
        sink.write_char('\n')?;
        sink.write_fmt(format_args!("変化：{}手\n", i))?;
        write_move_lines(fork, i, sink)?;
    }
    Ok(())
}

fn write_moves<W: Write>(moves: &[MoveFormat], sink: &mut W) -> Result {
    sink.write_str("手数----指手---------消費時間--\n")?;
    let Some((head, rest)) = moves.split_first() else {
        return Ok(());
    };
    if let Some(comments) = &head.comments {
        for comment in comments {
            if !comment.starts_with('&') {
                sink.write_char('*')?;
            }
            sink.write_str(comment)?;
            sink.write_char('\n')?;
        }
    }
    write_move_lines(rest, 1, sink)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::parse_jkf_file;
    use std::path::Path;

    // JKF lets a node carry comments and nothing else; KIF has no line for one.
    // Counting it as a ply anyway leaves a gap in the move numbers and moves
    // every `変化：N手` after it one step along, so the branch attaches to the
    // wrong move — or to none, and is dropped without a word.
    #[test]
    fn a_comment_only_node_does_not_consume_a_ply() {
        let jkf = JsonKifuFormat {
            initial: Some(Initial {
                preset: Preset::PresetHirate,
                data: None,
            }),
            moves: vec![
                MoveFormat::default(),
                pawn(Color::Black, 7, 7, 7, 6),
                MoveFormat {
                    comments: Some(vec!["ここで長考".to_string()]),
                    ..Default::default()
                },
                MoveFormat {
                    forks: Some(vec![vec![pawn(Color::White, 8, 3, 8, 4)]]),
                    ..pawn(Color::White, 3, 3, 3, 4)
                },
            ],
            ..Default::default()
        };
        let kif = jkf.to_kif_owned();
        assert!(
            kif.contains("   2 ３四歩(33)") && kif.contains("変化：2手"),
            "the comment must not push 3四歩 to ply 3: {kif:?}"
        );
        let back = crate::parser::parse_kif_str(&kif).expect("reads back");
        assert_eq!(
            1,
            back.moves
                .iter()
                .filter_map(|mf| mf.forks.as_ref())
                .map(Vec::len)
                .sum::<usize>(),
            "the branch survives: {kif:?}"
        );
    }

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

    #[test]
    fn to_kif_default() {
        assert_eq!(
            r#"
手数----指手---------消費時間--
"#[1..],
            JsonKifuFormat::default().to_kif_owned()
        );
    }

    #[test]
    fn fork_moves() {
        let path = Path::new("data/tests/kif/forks.json");
        let jkf = parse_jkf_file(path).expect("failed to parse kif");
        let kif = jkf.to_kif_owned();
        assert_eq!(
            &r#"
手数----指手---------消費時間--
   1 ７六歩(77)   ( 0:00/00:00:00)
   2 ８四歩(83)   ( 0:00/00:00:00)
   3 ６八銀(79)   ( 0:00/00:00:00)
   4 ３二金(41)   ( 0:00/00:00:00)
   5 ２六歩(27)   ( 0:00/00:00:00)
   6 ８五歩(84)   ( 0:00/00:00:00)
   7 ７七角(88)   ( 0:00/00:00:00)
   8 ３四歩(33)   ( 0:00/00:00:00)
   9 ７八金(69)   ( 0:00/00:00:00)
  10 ７七角成(22) ( 0:00/00:00:00)
  11 同　銀(68)   ( 0:00/00:00:00)
  12 ２二銀(31)   ( 0:00/00:00:00)

変化：10手
  10 ３三角(22)   ( 0:00/00:00:00)
  11 ６九玉(59)   ( 0:00/00:00:00)
  12 ４二銀(31)   ( 0:00/00:00:00)
  13 ３六歩(37)   ( 0:00/00:00:00)
  14 ７七角成(33) ( 0:00/00:00:00)

変化：5手
   5 ７七角(88)   ( 0:00/00:00:00)
   6 ３四歩(33)   ( 0:00/00:00:00)
   7 ４八銀(39)   ( 0:00/00:00:00)
   8 ６二銀(71)   ( 0:00/00:00:00)
   9 ３六歩(37)   ( 0:00/00:00:00)
  10 ８五歩(84)   ( 0:00/00:00:00)
  11 ７八金(69)   ( 0:00/00:00:00)
  12 ７四歩(73)   ( 0:00/00:00:00)

変化：9手
   9 １六歩(17)   ( 0:00/00:00:00)
  10 １四歩(13)   ( 0:00/00:00:00)
  11 ２六歩(27)   ( 0:00/00:00:00)
  12 ４二銀(31)   ( 0:00/00:00:00)
  13 ２二角成(77) ( 0:00/00:00:00)
  14 同　金(32)   ( 0:00/00:00:00)
  15 ７七銀(68)   ( 0:00/00:00:00)
"#[1..],
            kif.lines().skip(3).collect::<Vec<_>>().join("\n") + "\n"
        );
    }
}
