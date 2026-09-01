use super::WriteResult as Result;
use crate::error::ConvertError;
use crate::jkf::*;
use crate::notation::{board_word, KANSUJI, SANYOU_SUJI};
use std::collections::HashMap;
use std::fmt::Write;

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
        .ok_or(ConvertError::UnspellableNumber(num))?;
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
        return Err(ConvertError::UnspellableNumber(num));
    }
    if num > 10 {
        sink.write_char('十')?;
        num -= 10;
    }
    let c = num
        .checked_sub(1)
        .and_then(|i| KANSUJI.get(i as usize))
        .ok_or(ConvertError::UnspellableNumber(num))?;
    sink.write_char(*c)?;
    Ok(())
}

fn write_board_kind<W: Write>(kind: Kind, sink: &mut W) -> Result {
    sink.write_char(board_word(kind))?;
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
    // `その他` is the one name the table has no entry for — it is not a
    // handicap, it is "the board says it" (`handicap::lookup`).
    sink.write_str(crate::handicap::KIF_KEYWORD)?;
    sink.write_char('：')?;
    sink.write_str(crate::handicap::OTHER_NAME)?;
    sink.write_char('\n')?;
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
    // (R-KIF-014). Only `その他` needs it: a handicap's side to move follows
    // from the preset (R-HC-001).
    if data.color == Color::White {
        sink.write_str("後手番\n")?;
    }
    Ok(())
}

fn write_initial_preset<W: Write>(preset: Preset, sink: &mut W) -> Result {
    // A `その他` with a board never reaches here — the board is written instead.
    let name = match crate::handicap::lookup(preset) {
        Some(handicap) => handicap.kif_name,
        None => return Err(ConvertError::UnknownPreset(preset)),
    };
    sink.write_str(crate::handicap::KIF_KEYWORD)?;
    sink.write_char('：')?;
    sink.write_str(name)?;
    sink.write_char('\n')?;
    Ok(())
}

/// What ends a line, whichever of them a value carries.
///
/// R-CSA-001 leaves the newline to the environment, and a JKF built elsewhere
/// carries whatever that environment used — a lone `\r` among them. A value with
/// one inside it, written as it came, splits the line it was supposed to be on.
pub(super) const LINE_ENDS: [char; 2] = ['\n', '\r'];

/// `value` with everything that ends a line taken out.
///
/// For the places a format gives one line and no more: a header value in KIF
/// and KI2 (R-KIF-004) and in CSA (R-CSA-004). What is left of the line after a
/// newline inside the value is read as a line of its own, and the reader makes
/// of it whatever its shapes say — none of which is "the rest of that header".
pub(super) fn on_one_line(value: &str) -> String {
    value.split(LINE_ENDS).collect()
}

/// Writes one comment, as as many lines as it takes.
///
/// A comment is one line: `*` or `&` opens it and the newline ends it
/// (R-KIF-010 / R-KIF-011). A value carrying a newline of its own — JKF puts no
/// limit on one, and the consumer builds its own — leaves the text after it
/// outside any `*`, where the KIF reader skips it as a line it has no shape for
/// and the KI2 reader stops on it. The record either comes back short or does
/// not come back at all, and it is this crate's own writer that made the file.
///
/// Each line gets its own marker. `&` is a bookmark and marks a position rather
/// than the text (R-KIF-011), so only the line that carried it keeps it.
///
/// An empty comment is a `*` on its own, which is what the reader gives back for
/// one: dropping it instead would make a record the writer quietly shortens.
pub(super) fn write_comment<W: Write>(comment: &str, sink: &mut W) -> Result {
    for (i, line) in comment.split(LINE_ENDS).enumerate() {
        if i > 0 || !line.starts_with('&') {
            sink.write_char('*')?;
        }
        sink.write_str(line)?;
        sink.write_char('\n')?;
    }
    Ok(())
}

pub(super) fn write_header<W: Write>(header: &HashMap<String, String>, sink: &mut W) -> Result {
    for (k, v) in header {
        sink.write_str(k)?;
        sink.write_char('：')?;
        // R-KIF-004: a header is one line. A value carrying a newline — JKF puts
        // no limit on it, and the consumer builds its own headers — would end
        // the header block early, and the reader skips what follows as a
        // non-move line. Which header survives then depends on `HashMap`
        // iteration order, so the same record loses a different one each save.
        sink.write_str(&on_one_line(v))?;
        sink.write_char('\n')?;
    }
    Ok(())
}

/// Writes the line, or the board, that names the position the record starts
/// from (D13).
///
/// A record that leaves its starting position unsaid is read back as the even
/// game, which is what it means (`initial` is optional in JKF), but a file that
/// does not say so on its own is one nothing distinguishes from a save that was
/// cut short — and for KI2, where a hirate opening leaves nothing else to write,
/// a record with no header and no moves would be zero bytes.
///
/// `header` decides one case, and only one. A `手合割` this crate could not fold
/// into a `Preset` is kept as text (`手合割：詰将棋`, `parser::kakinoki`), which
/// leaves `preset` at the even game — not because the record says so, but
/// because that is what a reader takes from a line it cannot read. Writing the
/// even game under it makes the file say two different things about the one
/// thing this is here to state, and the fallback is exactly what leaving the
/// line out already means. A `手合割` that *is* a name this crate knows did not
/// come from that reader, so it does not get to speak for `initial` (D16).
///
/// A `preset` that names a handicap is not dropped for a header, and neither is
/// a board. Those say something the header does not, and a file that repeats
/// itself is better than one that reads back as a different game.
///
/// # Errors
///
/// [`ConvertError::UnknownPreset`] for `Initial { preset: PresetOther, data:
/// None }` — `その他` says the position is spelled out as a board, and without
/// one there is no word for it (`research/90-gaps.md` GAP-001). Every other
/// `Preset` has a name in `40-handicap.md`.
pub(super) fn write_initial<W: Write>(
    header: &HashMap<String, String>,
    initial: &Option<Initial>,
    sink: &mut W,
) -> Result {
    let preset = match initial {
        Some(Initial {
            data: Some(data), ..
        }) => return write_initial_data(data, sink),
        Some(initial) => initial.preset,
        // R-JKF-001: `initial` is optional, and its absence is the even game.
        None => Preset::PresetHirate,
    };
    // Only a `手合割` the reader could not fold into a `Preset` leaves `preset`
    // at the even game by default. A name this table knows would have been
    // folded, so finding one in `header` means the two came from different
    // places — and then the record's own is the one to write, or a handicap
    // turns into a hirate game where Black opens (R-HC-001) and the file no
    // longer reads back.
    if preset == Preset::PresetHirate
        && header
            .get(crate::handicap::KIF_KEYWORD)
            .is_some_and(|name| !crate::handicap::is_a_known_name(name))
    {
        return Ok(());
    }
    write_initial_preset(preset, sink)
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

    // A `手合割` whose value is not one of the handicaps in `40-handicap.md` is
    // kept as text (R-KIF-004) and `initial` falls back to the even game, which
    // is what the reader does with a record that names no handicap it knows.
    // Writing the preset line under it leaves the file saying two different
    // things about the one thing D13 is there to state.
    #[test]
    fn a_handicap_kept_as_text_is_not_written_over() {
        let kif = "手合割：詰将棋
先手：Ａ
手数----指手---------消費時間--
   1 ７六歩(77)
";
        let jkf = parse_kif_str(kif).expect("parses");
        for text in [
            jkf.try_to_kif_owned().expect("writes KIF"),
            jkf.try_to_ki2_owned().expect("writes KI2"),
        ] {
            assert_eq!(
                1,
                text.matches("手合割").count(),
                "one and only one: {text}"
            );
            assert!(
                text.contains("手合割：詰将棋"),
                "and it is the one read: {text}"
            );
        }
    }

    // The other way round, the header does not get to decide. A record that
    // names a handicap says something the header does not, and dropping the
    // line for it reads back as the even game — where Black moves first, not
    // the upper hand (R-HC-001), so every move changes sides.
    #[test]
    fn a_handicap_the_record_names_is_written_whatever_the_header_says() {
        let kif = "手合割：香落ち
手合割：詰将棋
手数----指手---------消費時間--
   1 ３四歩(33)
";
        let jkf = parse_kif_str(kif).expect("parses");
        let text = jkf.try_to_kif_owned().expect("writes KIF");
        assert!(text.contains("手合割：香落ち"), "{text}");
        let back = parse_kif_str(&text).expect("reads back");
        assert_eq!(jkf.initial, back.initial);
        assert_eq!(
            jkf.moves[1].move_.map(|mv| mv.color),
            back.moves[1].move_.map(|mv| mv.color),
            "R-HC-001: the upper hand moves first"
        );
    }

    // A comment is one line (R-KIF-010 / R-KIF-011). A JKF comment carrying a
    // newline of its own splits it, and what falls outside the `*` is skipped by
    // the KIF reader and stops the KI2 one — a file this crate wrote and cannot
    // read back. The header block next door has had the same rule for as long.
    #[test]
    fn a_comment_with_a_newline_in_it_is_written_as_lines() {
        use crate::jkf::*;
        let json = r#"{"header":{},"initial":{"preset":"HIRATE"},"moves":[{},
            {"move":{"color":0,"from":{"x":7,"y":7},"to":{"x":7,"y":6},"piece":"FU"},
             "comments":["囲い\n穴熊","&しおり\nつづき"]},
            {"move":{"color":1,"from":{"x":3,"y":3},"to":{"x":3,"y":4},"piece":"FU"}}]}"#;
        let jkf: JsonKifuFormat = serde_json::from_str(json).expect("reads the JKF");

        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        let back = parse_kif_str(&kif).expect("reads the KIF back");
        assert_eq!(2, back.moves.len() - 1, "{kif}");
        assert_eq!(
            Some(vec![
                "囲い".to_owned(),
                "穴熊".to_owned(),
                "&しおり".to_owned(),
                "つづき".to_owned(),
            ]),
            back.moves[1].comments,
            "{kif}"
        );

        // KI2 does not merely lose the tail: the line outside the `*` ends the
        // move list, and the record cannot be read back at all.
        let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
        let back = parse_ki2_str(&ki2).expect("reads the KI2 back");
        assert_eq!(2, back.moves.len() - 1, "{ki2}");
    }

    /// R-KIF-004: a header is one line. A value with a newline in it splits the
    /// header block, and everything after the split is skipped as a non-move
    /// line — a different header each time, because the order comes from a
    /// `HashMap`.
    #[test]
    fn a_header_value_never_breaks_the_header_block() {
        use crate::jkf::*;
        let mut header = std::collections::HashMap::new();
        header.insert("備考".to_owned(), "一行目\n二行目".to_owned());
        header.insert("棋戦".to_owned(), "テスト".to_owned());
        let jkf = JsonKifuFormat {
            header,
            initial: Some(Initial {
                preset: Preset::PresetHirate,
                data: None,
            }),
            moves: vec![MoveFormat::default()],
        };
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        let back = parse_kif_str(&kif).expect("reads back");
        assert_eq!(
            Some(&"テスト".to_owned()),
            back.header.get("棋戦"),
            "the header after the multi-line one survives: {kif:?}"
        );
        assert_eq!(2, back.header.len(), "{kif:?}");
    }

    /// The other side of the same line: the counts that *are* spellable. The
    /// rejection test alone leaves the writer free to stop writing counts
    /// altogether — 18 pawns would be saved as `歩` and 17 of them would be gone
    /// on the next read.
    ///
    /// 1 is written bare, 10 and 18 are the two-character spellings, and 2 is
    /// the ordinary case.
    #[test]
    fn a_hand_is_written_with_its_count_and_reads_back() {
        use crate::jkf::*;
        let mut state = StateFormat {
            color: Color::Black,
            board: [[Piece {
                color: None,
                kind: None,
            }; 9]; 9],
            hands: [Hand::default(); 2],
        };
        state.board[4][8] = Piece {
            color: Some(Color::Black),
            kind: Some(Kind::OU),
        };
        state.board[4][0] = Piece {
            color: Some(Color::White),
            kind: Some(Kind::OU),
        };
        state.hands[0] = Hand {
            FU: 18,
            KY: 1,
            KE: 10,
            GI: 2,
            KI: 0,
            KA: 0,
            HI: 0,
        };
        let jkf = JsonKifuFormat {
            initial: Some(Initial {
                preset: Preset::PresetOther,
                data: Some(state),
            }),
            moves: vec![MoveFormat::default()],
            ..Default::default()
        };
        let kif = jkf.try_to_kif_owned().expect("writes KIF");
        let line = kif
            .lines()
            .find(|l| l.starts_with("先手の持駒："))
            .unwrap_or_else(|| panic!("no hand line in {kif:?}"));
        for want in ["歩十八", "香", "桂十", "銀二"] {
            assert!(line.contains(want), "{want} missing from {line:?}");
        }
        assert!(!line.contains("香一"), "1 is written bare: {line:?}");
        assert_eq!(
            jkf.initial,
            parse_kif_str(&kif).expect("reads back").initial,
            "{kif:?}"
        );
    }

    /// Coordinates and hand counts come from the record, so they can be out of
    /// range. Spelling one is an error the caller can see, not an index into a
    /// table that takes the process down.
    ///
    /// Which one it was has to survive to the caller. The consumer saves a game
    /// through here and has to tell the user what went wrong; "an error occurred
    /// when formatting an argument" — all `std::fmt::Error` can say — is not
    /// something anyone can act on.
    #[test]
    fn unspellable_records_are_errors() {
        use crate::error::ConvertError;
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
        assert_eq!(
            ConvertError::UnspellableNumber(0),
            bad_square
                .try_to_kif_owned()
                .expect_err("(0, 0) is not a square")
        );

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
            assert_eq!(
                ConvertError::UnspellableNumber(count),
                too_many
                    .try_to_kif_owned()
                    .expect_err("{count} pieces in hand should not be spellable"),
                "{count} pieces in hand"
            );
        }
    }

    // A board without `後手番` reads back as Black to move, and the first move
    // then fails to apply. Writing the line is this crate's job (D6): a
    // consumer patching it in afterwards would write it twice.
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

        // The other direction. Writing `後手番` unconditionally would pass every
        // assertion above, and then a Black-to-move position would come back
        // with the sides swapped — the same bug as a missing line, opened the
        // other way round.
        // The moves are dropped: they are White's, and they stop being legal
        // once the board says Black. The board alone is what is under test.
        let mut black_to_move = crate::jkf::JsonKifuFormat {
            moves: vec![crate::jkf::MoveFormat::default()],
            ..jkf.clone()
        };
        if let Some(data) = black_to_move.initial.as_mut().and_then(|i| i.data.as_mut()) {
            data.color = crate::jkf::Color::Black;
        }
        let kif = black_to_move.try_to_kif_owned().expect("writes KIF");
        let ki2 = black_to_move.try_to_ki2_owned().expect("writes KI2");
        for text in [&kif, &ki2] {
            assert!(
                !text.contains("後手番"),
                "a Black-to-move board must not say 後手番: {text:?}"
            );
        }
        assert_eq!(
            black_to_move.initial,
            parse_kif_str(&kif).expect("reads back").initial
        );
        assert_eq!(
            black_to_move.initial,
            parse_ki2_str(&ki2).expect("reads back").initial
        );
    }
}
