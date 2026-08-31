use super::kakinoki::{write_header, write_initial, write_kansuji, write_sanyou_suji};
use super::WriteResult as Result;
use crate::error::ConvertError;
use crate::jkf::*;
use std::fmt::Write;

/// A type that is convertible to KIF format.
pub trait ToKif {
    /// Write `self` in KIF format.
    ///
    /// # Errors
    ///
    /// Returns [`ConvertError::Write`] if `sink` fails, and otherwise the
    /// variant naming what KIF cannot spell: [`InvalidSquare`] for a coordinate
    /// outside the board, [`UnspellableNumber`] for more pieces in hand than a
    /// set contains, [`UnknownPreset`] for a handicap with no board. A
    /// caller that has just failed to save a game has to tell these apart —
    /// they are different things to say to the user.
    ///
    /// [`InvalidSquare`]: ConvertError::InvalidSquare
    /// [`UnspellableNumber`]: ConvertError::UnspellableNumber
    /// [`UnknownPreset`]: ConvertError::UnknownPreset
    fn to_kif<W: Write>(&self, sink: &mut W) -> Result;

    /// Returns `self`'s string representation, or the error [`Self::to_kif`]
    /// gave.
    fn try_to_kif_owned(&self) -> std::result::Result<String, ConvertError> {
        let mut s = String::new();
        self.to_kif(&mut s)?;
        Ok(s)
    }

    /// Returns `self`'s string representation.
    ///
    /// A record that cannot be spelled in KIF yields whatever was written
    /// before the failure. Use [`Self::try_to_kif_owned`] to see the error
    /// instead of a truncated file.
    #[deprecated(
        note = "returns a truncated string on failure, which the caller writes to disk as if it were the whole record. Use try_to_kif_owned."
    )]
    fn to_kif_owned(&self) -> String {
        let mut s = String::new();
        let _ = self.to_kif(&mut s);
        s
    }
}

impl ToKif for JsonKifuFormat {
    fn to_kif<W: Write>(&self, sink: &mut W) -> Result {
        write_header(&self.header, sink)?;
        write_initial(&self.header, &self.initial, sink)?;
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
        let has_line = mf.occupies_a_ply();
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
                // A `from` off the board is either an unresolved
                // `normalizer::ORIGIN_UNSTATED` or a coordinate from a broken
                // record. `(00)` is not a KIF origin (R-KIF-006) and this
                // crate's own reader takes it straight back as the sentinel, so
                // writing it produces a corruption that round-trips forever.
                shogi_core::Square::try_from(&from)?;
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
                // R-KIF-008 spells the move's own time as 分:秒 with no stated
                // limit on the minutes, so an hour folds into them. Writing
                // `time.now.m` alone drops it, and the file then disagrees with
                // its own running total: CSA's `T3723` came out as
                // `( 2:03/01:02:03)`.
                let minutes =
                    u32::from(time.now.h.unwrap_or_default()) * 60 + u32::from(time.now.m);
                sink.write_fmt(format_args!(
                    "({:2}:{:02}/{:02}:{:02}:{:02})",
                    minutes,
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
    use crate::converter::{ToCsa, ToKi2};
    use crate::parser::{parse_jkf_file, parse_kif_str};
    use std::path::Path;

    /// A board with only the two kings, `{hands}` in Black's hand, and `{mv}`
    /// as the single move.
    fn one_move_kif(hands: &str, extra: &str, mv: &str) -> String {
        format!(
            "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
|v玉 ・ ・ ・ ・ ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ ・ ・{extra} ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| 玉 ・ ・ ・ ・ ・ ・ ・ ・|九
+---------------------------+
先手の持駒：{hands}
先手番
手数----指手---------消費時間--
   1 {mv}
"
        )
    }

    // R-KIF-006: KIF puts 打 on every drop and never writes 不成 — the exact
    // opposite of the traditional notation KI2 uses (R-NOT-005). Getting either
    // backwards produces a line this crate's own reader cannot parse, and the
    // record comes back with no moves at all.
    #[test]
    fn kif_marks_every_drop_and_never_writes_不成() {
        // A bishop in hand and none on the board: R-NOT-003 would leave 打 off,
        // R-KIF-006 requires it.
        let jkf = parse_kif_str(&one_move_kif("角", " ・", "４五角打")).expect("parses");
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        assert!(kif.contains("４五角打"), "{kif:?}");
        assert_eq!(
            1,
            parse_kif_str(&kif).expect("reads back").moves.len() - 1,
            "the drop survives: {kif:?}"
        );

        // A silver on 3三 stepping to 3二 without promoting. `normalize` records
        // `promote: Some(false)`; KIF must not spell it.
        let jkf = parse_kif_str(&one_move_kif("なし", " 銀", "３二銀(33)")).expect("parses");
        assert_eq!(
            Some(false),
            jkf.moves[1].move_.expect("a move").promote,
            "the move declines promotion"
        );
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        assert!(!kif.contains("不成"), "R-KIF-006: {kif:?}");
        assert_eq!(
            1,
            parse_kif_str(&kif).expect("reads back").moves.len() - 1,
            "the move survives: {kif:?}"
        );
        // R-NOT-005 is the other way round, and the same JKF has to show it —
        // one side alone does not pin the difference between the two formats.
        assert!(jkf
            .try_to_ki2_owned()
            .expect("writes KI2")
            .contains("▲３二銀不成"));
    }

    // GAP-015: `(00)` is not a KIF origin (R-KIF-005), and this crate's own
    // reader takes it back as the marker for an unstated one — so a record
    // written with it round-trips as a stable corruption. CSA reads the same
    // digits as the hand (R-CSA-007), turning a board move into a drop.
    #[test]
    fn an_origin_off_the_board_is_never_spelled() {
        let jkf = JsonKifuFormat {
            initial: Some(Initial {
                preset: Preset::PresetHirate,
                data: None,
            }),
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        from: Some(PlaceFormat { x: 0, y: 0 }),
                        ..pawn(Color::Black, 7, 7, 7, 6).move_.expect("a move")
                    }),
                    ..Default::default()
                },
            ],
            ..Default::default()
        };
        assert!(
            jkf.try_to_kif_owned().is_err(),
            "{:?}",
            jkf.try_to_kif_owned()
        );
        assert!(
            jkf.try_to_csa_owned().is_err(),
            "{:?}",
            jkf.try_to_csa_owned()
        );
    }

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
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
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
手合割：平手
手数----指手---------消費時間--
"#[1..],
            JsonKifuFormat::default()
                .try_to_kif_owned()
                .expect("writes KIF")
        );
    }

    #[test]
    fn fork_moves() {
        let path = Path::new("data/tests/kif/forks.json");
        let jkf = parse_jkf_file(path).expect("failed to parse kif");
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
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
