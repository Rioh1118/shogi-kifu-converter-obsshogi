use crate::jkf::*;
use std::collections::HashMap;
use std::fmt::{Result, Write};

const SANYOU_SUJI: [char; 9] = ['１', '２', '３', '４', '５', '６', '７', '８', '９'];
const KANSUJI: [char; 10] = ['一', '二', '三', '四', '五', '六', '七', '八', '九', '十'];

/// Writes a file as a full-width digit.
///
/// `num` comes from the record, so a value outside 1-9 is a broken input rather
/// than something to index the table with. The KIF parser leaves `to` at (0, 0)
/// for a `同` move until the position fills it in, so an unnormalised record
/// reaches here with a zero.
pub(super) fn write_sanyou_suji<W: Write>(num: u8, sink: &mut W) -> Result {
    let c = num
        .checked_sub(1)
        .and_then(|i| SANYOU_SUJI.get(i as usize))
        .ok_or(std::fmt::Error)?;
    sink.write_char(*c)?;
    Ok(())
}

/// Writes a number 1-18 in kanji.
///
/// Ranks are 1-9 and hand counts reach 18 for pawns. Anything past that has no
/// spelling here — and writing `歩十十` would produce a file this crate's own
/// parser cannot read back.
pub(super) fn write_kansuji<W: Write>(mut num: u8, sink: &mut W) -> Result {
    if num > 18 {
        return Err(std::fmt::Error);
    }
    if num > 10 {
        sink.write_char('十')?;
        num -= 10;
    }
    let c = num
        .checked_sub(1)
        .and_then(|i| KANSUJI.get(i as usize))
        .ok_or(std::fmt::Error)?;
    sink.write_char(*c)?;
    Ok(())
}

fn write_board_kind<W: Write>(kind: Kind, sink: &mut W) -> Result {
    match kind {
        Kind::FU => sink.write_char('歩')?,
        Kind::KY => sink.write_char('香')?,
        Kind::KE => sink.write_char('桂')?,
        Kind::GI => sink.write_char('銀')?,
        Kind::KI => sink.write_char('金')?,
        Kind::KA => sink.write_char('角')?,
        Kind::HI => sink.write_char('飛')?,
        Kind::OU => sink.write_char('玉')?,
        Kind::TO => sink.write_char('と')?,
        Kind::NY => sink.write_char('杏')?,
        Kind::NK => sink.write_char('圭')?,
        Kind::NG => sink.write_char('全')?,
        Kind::UM => sink.write_char('馬')?,
        Kind::RY => sink.write_char('龍')?,
    }
    Ok(())
}

fn write_hand<W: Write>(hand: &Hand, sink: &mut W) -> Result {
    for (c, num) in [
        ('飛', hand.HI),
        ('角', hand.KA),
        ('金', hand.KI),
        ('銀', hand.GI),
        ('桂', hand.KE),
        ('香', hand.KY),
        ('歩', hand.FU),
    ] {
        if num > 0 {
            sink.write_char(c)?;
            if num > 1 {
                write_kansuji(num, sink)?;
            }
            sink.write_char('　')?;
        }
    }
    Ok(())
}

fn write_initial_data<W: Write>(data: &StateFormat, sink: &mut W) -> Result {
    sink.write_str("手合割：その他\n")?;
    sink.write_str("後手の持駒：")?;
    if data.hands[1] != Hand::default() {
        write_hand(&data.hands[1], sink)?;
    } else {
        sink.write_str("なし")?;
    }
    sink.write_char('\n')?;
    sink.write_str("  ９ ８ ７ ６ ５ ４ ３ ２ １\n")?;
    sink.write_str("+---------------------------+\n")?;
    for i in 0..9 {
        sink.write_char('|')?;
        for j in 0..9 {
            let p = data.board[8 - j][i];
            if let (Some(c), Some(kind)) = (p.color, p.kind) {
                match c {
                    Color::Black => sink.write_char(' ')?,
                    Color::White => sink.write_char('v')?,
                };
                write_board_kind(kind, sink)?;
            } else {
                sink.write_str(" ・")?;
            }
        }
        sink.write_char('|')?;
        write_kansuji(i as u8 + 1, sink)?;
        sink.write_char('\n')?;
    }
    sink.write_str("+---------------------------+\n")?;
    sink.write_str("先手の持駒：")?;
    if data.hands[0] != Hand::default() {
        write_hand(&data.hands[0], sink)?;
    } else {
        sink.write_str("なし")?;
    }
    sink.write_char('\n')?;
    // Without this line a reader takes the position as Black to move, so a
    // tsume or a study starting from White fails on its very first move
    // (R-KIF-009). Only `その他` needs it: a handicap's side to move follows
    // from the preset (R-HC-001).
    if data.color == Color::White {
        sink.write_str("後手番\n")?;
    }
    Ok(())
}

fn write_initial_preset<W: Write>(preset: Preset, sink: &mut W) -> Result {
    // `その他` never reaches here — it carries a board instead.
    let name = match crate::handicap::lookup(preset) {
        Some(handicap) => handicap.kif_name,
        None => return Err(std::fmt::Error),
    };
    sink.write_str("手合割：")?;
    sink.write_str(name)?;
    sink.write_char('\n')?;
    Ok(())
}

pub(super) fn write_header<W: Write>(header: &HashMap<String, String>, sink: &mut W) -> Result {
    for (k, v) in header {
        sink.write_str(k)?;
        sink.write_char('：')?;
        sink.write_str(v)?;
        sink.write_char('\n')?;
    }
    Ok(())
}

pub(super) fn write_initial<W: Write>(
    initial: &Option<Initial>,
    omit_hirate: bool,
    sink: &mut W,
) -> Result {
    if let Some(initial) = initial {
        if let Some(data) = &initial.data {
            write_initial_data(data, sink)?;
        } else {
            if omit_hirate && initial.preset == Preset::PresetHirate {
                return Ok(());
            }
            write_initial_preset(initial.preset, sink)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::converter::{ToKi2, ToKif};
    use crate::parser::{parse_ki2_str, parse_kif_str};

    /// A tsume position with White to move: the lone Black king on 5i, a White
    /// king on 5a and a White rook on 5b.
    const GOTE_TSUME: &str = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・v飛 ・ ・ ・ ・|二
| ・ ・ ・ ・ ・ ・ ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
先手の持駒：なし
後手番
手数----指手---------消費時間--
   1 ５三飛(52)   ( 0:00/00:00:00)
";

    /// Coordinates and hand counts come from the record, so they can be out of
    /// range. Writing used to index a table with them and take the process
    /// down; now it is an error the caller can see.
    #[test]
    fn unspellable_records_are_errors() {
        use crate::jkf::*;
        let bad_square = JsonKifuFormat {
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: None,
                        to: PlaceFormat { x: 0, y: 0 },
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
        assert!(bad_square.try_to_kif_owned().is_err());

        let mut state = StateFormat {
            color: Color::Black,
            board: [[Piece {
                color: None,
                kind: None,
            }; 9]; 9],
            hands: [Hand::default(); 2],
        };
        // 18 is every pawn on the board. 19 and 20 are the values the `> 18`
        // guard exists for: `十九` and `二十` do have kanji spellings, so
        // dropping the guard writes `先手の持駒：歩十九`, which this crate's own
        // parser reads back as no pieces in hand at all. 21 is caught either
        // way, so a test that only tries 21 does not see the guard.
        for count in [19u8, 20, 21] {
            state.hands[0].FU = count;
            let too_many = JsonKifuFormat {
                initial: Some(Initial {
                    preset: Preset::PresetOther,
                    data: Some(state),
                }),
                ..Default::default()
            };
            assert!(
                too_many.try_to_kif_owned().is_err(),
                "{count} pieces in hand should not be spellable"
            );
        }
    }

    // A board without `後手番` reads back as Black to move, and the first move
    // then fails to apply. The consumer used to patch the line in by hand after
    // the fact, which is this crate's job (R-KIF-009).
    #[test]
    fn side_to_move_survives_a_round_trip() {
        let jkf = parse_kif_str(GOTE_TSUME).expect("parses");
        let color = jkf.initial.and_then(|i| i.data).expect("a board").color;
        assert_eq!(crate::jkf::Color::White, color);

        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        assert!(kif.contains("後手番"), "no side to move in {kif:?}");
        let back = parse_kif_str(&kif).expect("reads back");
        assert_eq!(jkf.initial, back.initial);

        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        assert!(ki2.contains("後手番"), "no side to move in {ki2:?}");
        let back = parse_ki2_str(&ki2).expect("reads back");
        assert_eq!(jkf.initial, back.initial);
    }
}
