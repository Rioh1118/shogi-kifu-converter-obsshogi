//! Parsers for [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)

mod kakinoki;

mod ki2;
mod kif;

use crate::error::ParseError;
use crate::jkf::JsonKifuFormat;
use encoding_rs::{Encoding, SHIFT_JIS, UTF_8};
use nom::error::convert_error;
use nom::Finish;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Parses a CSA file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// The bytes decide the encoding, as they do for KIF and KI2: UTF-8 first, then
/// Shift-JIS. R-CSA-001 leaves the encoding to the environment that wrote the
/// file, and the Windows GUIs that write most CSA in the wild write Shift-JIS.
///
/// The extension is not consulted. R-CSA-001 names one (`csa`) but leaves the
/// encoding to the environment, and the extension is only ever a hint at the
/// encoding here (`read_kifu`) — with nothing to hint at, there is nothing to
/// refuse a file for.
///
/// # Errors
///
/// Returns [`ParseError::Decode`] when neither encoding reads the bytes cleanly,
/// [`ParseError::Io`] when the file cannot be read, and otherwise whatever
/// [`parse_csa_str`] returns.
///
/// # Panics
///
/// Panics on the inputs [`parse_csa_str`] panics on. Every file this decodes
/// reaches that parser, in either encoding.
pub fn parse_csa_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let mut buf = Vec::new();
    File::open(&path)?.read_to_end(&mut buf)?;
    parse_csa_str(&decode_kifu(&buf, UTF_8)?)
}

/// Parses a CSA formatted string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ParseError`] if it fails to parse the string.
///
/// # Panics
///
/// The body goes to the `csa` crate, which unwraps rather than reports on some
/// values it cannot read: a `$START_TIME` naming a day no month has
/// (`2004/02/30`), a `T` line with more digits than the number it holds. Both
/// are things a file can say, so this is reachable from input alone — the
/// consumer catches it (obs-shogi's `parse_csa_guarded`) because a Tauri command
/// that panics takes the application with it. `research/90-gaps.md` GAP-012
/// holds what it would take to fix rather than catch.
pub fn parse_csa_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let mut jkf = JsonKifuFormat::try_from(csa::parse_csa(without_a_byte_order_mark(s))?)?;
    jkf.normalize()?;
    Ok(jkf)
}

/// Parses a KIF file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// The extension chooses which encoding to try first — Shift-JIS for `.kif`,
/// UTF-8 for `.kifu` — and the bytes get the last word: if that decode raises a
/// replacement character the other encoding is tried (R-REQ-003, D14). A `.kif`
/// holding UTF-8 reads, and so does a `.kifu` holding Shift-JIS. What the
/// extension does decide is whether the file is offered at all: anything but
/// those two is refused unread.
///
/// See: [http://kakinoki.o.oo7.jp/kif_format.html](http://kakinoki.o.oo7.jp/kif_format.html)
///
/// # Errors
///
/// Returns [`ParseError::FileExtension`] for an extension other than `.kif` or
/// `.kifu`, [`ParseError::Decode`] when neither encoding reads the bytes
/// cleanly, [`ParseError::Io`] when the file cannot be read, and otherwise
/// whatever [`parse_kif_str`] returns.
pub fn parse_kif_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let text = read_kifu(path, &[("kif", SHIFT_JIS), ("kifu", UTF_8)])?;
    parse_kif_str(&text)
}

/// Reads `path` as text, picking the encoding from its extension and the bytes.
///
/// `extensions` maps each extension this reader claims to the encoding it names.
///
/// R-REQ-003: the extension names an encoding but does not guarantee one. A
/// `.kif` holding UTF-8 and a `.ki2` holding Shift-JIS both turn up, so the
/// bytes get the last word — a decode that reports errors is not the one the
/// file was written in.
///
/// Deciding on the decode rather than on a failed parse is what makes this
/// reachable at all. The full-width forms a kifu is written in (`７`, `：`) sit
/// at U+FF00 and up, whose UTF-8 lead byte is `0xEF`; Shift-JIS has nothing
/// there, so a UTF-8 file read as Shift-JIS fails at the decode and never gets
/// as far as a parse error to retry on.
fn read_kifu<P: AsRef<Path>>(
    path: P,
    extensions: &[(&str, &'static Encoding)],
) -> Result<String, ParseError> {
    // Case-folded: a file saved on Windows arrives as `.KIF` just as often as
    // `.kif`, and the extension is only a hint at the encoding to begin with.
    let ext = path
        .as_ref()
        .extension()
        .and_then(std::ffi::OsStr::to_str)
        .map(str::to_ascii_lowercase)
        .ok_or(ParseError::FileExtension)?;
    let named = extensions
        .iter()
        .find(|(name, _)| *name == ext)
        .map(|(_, encoding)| *encoding)
        .ok_or(ParseError::FileExtension)?;
    let mut buf = Vec::new();
    File::open(&path)?.read_to_end(&mut buf)?;
    decode_kifu(&buf, named)
}

/// Decodes `buf` as `first`, falling back to the other of the two encodings a
/// kifu is written in.
///
/// A decode that reports errors is not the one the file was written in, so the
/// first clean decode wins. `first` is what the caller has reason to expect —
/// the extension for KIF and KI2, UTF-8 for CSA — and only breaks the tie for
/// bytes both encodings accept.
fn decode_kifu(buf: &[u8], first: &'static Encoding) -> Result<String, ParseError> {
    let second = if first == SHIFT_JIS { UTF_8 } else { SHIFT_JIS };
    for encoding in [first, second] {
        let (text, _, had_errors) = encoding.decode(buf);
        if !had_errors {
            return Ok(text.into_owned());
        }
    }
    Err(ParseError::Decode)
}

/// The error for input the reader stopped on, with `rest` located in `whole`.
///
/// D1: a reader that stops early has to say so. Returning `Ok` with the record
/// truncated is the worst of the three outcomes — the caller cannot tell it
/// from a short game, so obs-shogi indexes a fraction of the moves and nobody
/// finds out (GAP-005: 79% of `bug_big.kif` went missing this way).
///
/// It also decides an encoding: the consumer tries encodings in turn and takes
/// the first `Ok`, so a mojibake decode that yields an empty record used to win
/// over the decode that would have read the game.
fn stopped_at(whole: &str, rest: &str) -> String {
    convert_error(
        whole,
        nom::error::VerboseError {
            errors: vec![(
                rest,
                nom::error::VerboseErrorKind::Context("cannot read this"),
            )],
        },
    )
}

/// Whether `jkf` holds nothing the reader actually recognised.
///
/// Leftover input is not enough on its own. A line the move list has no shape
/// for is skipped whole, so an input made only of such lines — a CSA file
/// renamed to `.kif`, a JSON, a mojibake decode with no newline in it — leaves
/// nothing behind to report and comes back as an empty record.
///
/// That empty record is worse than an error, because the consumer picks a text
/// encoding on this answer: obs-shogi tries encodings in turn and takes the
/// first `Ok`, so `bug_big.kif` — which D1 means to reject at its unreadable
/// line — instead came back as a UTF-16LE mojibake holding zero moves, and
/// *that* won. The error D1 exists to raise never reached anyone.
///
/// A header-only kifu is not this: it fills in `header` or `initial`.
fn recognised_nothing(jkf: &JsonKifuFormat, read_header: bool) -> bool {
    !read_header
        && jkf
            .moves
            .iter()
            .all(|mf| mf.move_.is_none() && mf.special.is_none() && mf.comments.is_none())
}

/// Drops a byte-order mark from the head of a record.
///
/// A BOM is not part of the text, and every reader below takes what it is given
/// literally: left on, it goes into the first line — `\u{feff}手合割：香落ち` is
/// filed under a key nobody wrote, the handicap is lost, and the record reads as
/// 平手 with every side reversed (R-HC-001 / R-RULE-006). The rest of the file
/// reads, so nothing downstream can tell that the first line went
/// (`research/90-gaps.md` GAP-006). Shogidokoro writes one.
fn without_a_byte_order_mark(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// Parses a KIF formatted string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// Returns [`ParseError::Kif`] when the reader stops before the end of `s` —
/// a numbered line whose word is not in the KIF vocabulary (R-KIF-007) is the
/// usual cause — or when nothing in `s` was recognised as a kifu at all (D1).
/// Returns [`ParseError::Normalize`] when a move cannot be played from the
/// position before it.
pub fn parse_kif_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let s = without_a_byte_order_mark(s);
    match kif::parse(s).finish() {
        Ok((rest, (mut jkf, read_header))) => {
            if !rest.trim().is_empty() {
                return Err(ParseError::Kif(stopped_at(s, rest)));
            }
            if !s.trim().is_empty() && recognised_nothing(&jkf, read_header) {
                return Err(ParseError::Kif(stopped_at(s, s)));
            }
            // KIF moves carry an explicit `from`, so `relative` inference is dead work.
            // Downstream consumers (e.g. KI2 conversion) can opt-in via `populate_relative()`.
            jkf.normalize_with_options(true, false)?;
            Ok(jkf)
        }
        Err(err) => Err(ParseError::Kif(convert_error(s, err))),
    }
}

/// Parses a KI2 file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// The extension chooses which encoding to try first — Shift-JIS for `.ki2`,
/// UTF-8 for `.ki2u` — and the bytes get the last word (R-REQ-003, D14), the
/// same as [`parse_kif_file`]. What the extension decides is whether the file
/// is offered at all: anything but those two is refused unread.
///
/// See: [http://kakinoki.o.oo7.jp/KifuwInt.htm](http://kakinoki.o.oo7.jp/KifuwInt.htm)
///
/// # Errors
///
/// Returns [`ParseError::FileExtension`] for an extension other than `.ki2` or
/// `.ki2u`, [`ParseError::Decode`] when neither encoding reads the bytes
/// cleanly, [`ParseError::Io`] when the file cannot be read, and otherwise
/// whatever [`parse_ki2_str`] returns.
pub fn parse_ki2_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let text = read_kifu(path, &[("ki2", SHIFT_JIS), ("ki2u", UTF_8)])?;
    parse_ki2_str(&text)
}

/// Parses a KI2 formatted string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// Returns [`ParseError::Ki2`] when the reader stops before the end of `s`, or
/// when nothing in `s` was recognised as a kifu at all (D1). Returns
/// [`ParseError::Normalize`] when a move cannot be played from the position
/// before it.
pub fn parse_ki2_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let s = without_a_byte_order_mark(s);
    match ki2::parse(s).finish() {
        Ok((rest, (mut jkf, read_header))) => {
            if !rest.trim().is_empty() {
                return Err(ParseError::Ki2(stopped_at(s, rest)));
            }
            if !s.trim().is_empty() && recognised_nothing(&jkf, read_header) {
                return Err(ParseError::Ki2(stopped_at(s, s)));
            }
            jkf.normalize_with_color_correction(true)?;
            Ok(jkf)
        }
        Err(err) => Err(ParseError::Ki2(convert_error(s, err))),
    }
}

/// Parses a JSON file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ParseError`] if it fails to parse the file.
pub fn parse_jkf_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    // Through the string reader, like every other format, so that what is
    // decided about the bytes is decided in one place: a byte-order mark left on
    // makes `serde_json` refuse the file, and the rule that removes it lives
    // there (`without_a_byte_order_mark`). This is the entry the consumer's
    // index feeds (R-REQ-002).
    parse_jkf_str(&std::fs::read_to_string(path)?)
}

/// Parses a JSON string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ParseError`] if it fails to parse the string.
pub fn parse_jkf_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let mut jkf = serde_json::from_str::<JsonKifuFormat>(without_a_byte_order_mark(s))?;
    jkf.normalize()?;
    Ok(jkf)
}

#[cfg(test)]
mod tests {
    use std::io::BufReader;
    // R-KIF-001 / GAP-006: a BOM is not part of the record. Left on, it joins
    // the first line — the handicap goes missing and the record reads as 平手
    // with every side reversed (R-HC-001) — while everything below it reads, so
    // nothing downstream can tell. Shogidokoro writes one.
    #[test]
    fn a_byte_order_mark_is_not_part_of_the_record() {
        const KIF: &str =
            "手合割：香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n   2 ７六歩(77)\n";
        const KI2: &str = "手合割：香落ち\n△３四歩 ▲７六歩\n";
        const CSA: &str = "V2.2\nPI\n+\n+7776FU\n";
        assert_eq!(
            parse_kif_str(KIF).expect("reads"),
            parse_kif_str(&format!("\u{feff}{KIF}")).expect("reads with a BOM")
        );
        assert_eq!(
            parse_ki2_str(KI2).expect("reads"),
            parse_ki2_str(&format!("\u{feff}{KI2}")).expect("reads with a BOM")
        );
        assert_eq!(
            parse_csa_str(CSA).expect("reads"),
            parse_csa_str(&format!("\u{feff}{CSA}")).expect("reads with a BOM")
        );
        // Including from a file, which is the entry the consumer's index uses
        // for JKF (R-REQ-002) and the one place the rule was not applied.
        let jkf = r#"{"header":{},"initial":{"preset":"HIRATE"},"moves":[{}]}"#;
        assert_eq!(
            parse_jkf_file(scratch("plain.jkf", jkf.as_bytes())).expect("reads"),
            parse_jkf_file(scratch("bom.jkf", format!("\u{feff}{jkf}").as_bytes()))
                .expect("reads with a BOM")
        );
    }

    // `parse_jkf_str` is the entry the consumer's save path feeds (R-REQ-002)
    // and it had no test at all — GAP-013 was a fault only JKF-in could reach.
    //
    // A move with no origin is a drop (R-JKF-003), not a move whose origin has
    // to be worked out. The position here is built so the difference shows: a
    // bishop on the board reaches the square the bishop in hand is dropped on,
    // so looking the origin up succeeds and quietly moves the wrong piece —
    // taking it off the board and leaving the hand as it was.
    #[test]
    fn jkf_keeps_a_drop_a_drop() {
        let mut board = [[Piece::empty(); 9]; 9];
        let mut place = |x: usize, y: usize, color, kind| {
            board[x - 1][y - 1] = Piece {
                color: Some(color),
                kind: Some(kind),
            };
        };
        place(5, 1, Color::White, Kind::OU);
        place(5, 9, Color::Black, Kind::OU);
        place(8, 8, Color::White, Kind::KA);
        let mut hands = [Hand::empty(); 2];
        hands[Color::White as usize].KA = 1;
        let jkf = JsonKifuFormat {
            header: Default::default(),
            initial: Some(Initial {
                preset: Preset::PresetOther,
                data: Some(StateFormat {
                    color: Color::White,
                    board,
                    hands,
                }),
            }),
            moves: vec![
                MoveFormat::default(),
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: None,
                        to: PlaceFormat { x: 5, y: 5 },
                        piece: Kind::KA,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                },
            ],
        };
        let json = serde_json::to_string(&jkf).expect("serializes");
        let parsed = super::parse_jkf_str(&json).expect("parses");
        let mv = parsed.moves[1].move_.expect("a move");
        assert_eq!(None, mv.from, "a drop has no origin to fill in");
        assert_eq!(Kind::KA, mv.piece);
        assert_eq!(None, mv.capture, "a drop takes nothing");
    }

    // R-HC-001: at a handicap the opening move is White's, so a producer that
    // took the side from the ply number has every colour in the record the wrong
    // way round. JKF states the colour rather than deriving it, and nothing
    // later in normalization consults the ply number, so the whole record is
    // turned over up front on the strength of the first move alone.
    #[test]
    fn jkf_turns_over_a_handicap_numbered_from_black() {
        let record = |color: u8| {
            format!(
                r#"{{"header":{{}},"initial":{{"preset":"KY"}},"moves":[{{}},
                   {{"move":{{"color":{color},"from":{{"x":3,"y":3}},"to":{{"x":3,"y":4}},"piece":"FU"}}}},
                   {{"move":{{"color":{},"from":{{"x":7,"y":7}},"to":{{"x":7,"y":6}},"piece":"FU"}}}}]}}"#,
                1 - color
            )
        };
        for numbered_from in [0, 1] {
            let jkf = super::parse_jkf_str(&record(numbered_from)).expect("parses");
            assert_eq!(
                [Color::White, Color::Black],
                [1, 2].map(|i| jkf.moves[i].move_.expect("a move").color),
                "numbered from {numbered_from}"
            );
        }
    }

    // D1: a reader that stops early has to say so. Returning `Ok` with the
    // record truncated is the worst of the three outcomes — the caller cannot
    // tell it from a short game. `bug_big.kif` lost 79% of its moves this way
    // and reported success (GAP-005).
    //
    // The word that stopped the reader is the one thing it cannot recover, so
    // the error has to carry it and point at its own line.
    #[test]
    fn a_record_the_reader_stops_in_the_middle_of_is_an_error() {
        const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n";
        for (tail, line, word) in [
            // `パス` is a real token in files written by analysis software, and
            // it is not in R-KIF-007's vocabulary. JKF has no `MoveSpecial` that
            // means "the turn passes" and neither does tsshogi's JKF, so there
            // is nothing to read it into — D8.
            ("   1 ７六歩(77)\n   2 パス\n   3 ２六歩(27)\n", 4, "パス"),
            // A KI2 move line in a KIF. tsshogi rejects this too.
            ("   1 ７六歩(77)\n▲２六歩\n   2 ３四歩(33)\n", 4, "▲２六歩"),
            // A numbered line whose word is nothing the format has.
            ("   1 ７六歩(77)\n   2 ほげ\n", 4, "ほげ"),
        ] {
            let err =
                parse_kif_str(&format!("{HEAD}{tail}")).expect_err("{word} should stop the reader");
            let text = err.to_string();
            assert!(text.contains(word), "{word} is missing from {text:?}");
            assert!(
                text.contains(&format!("at line {line}")),
                "{word} should point at line {line}: {text:?}"
            );
        }
    }

    /// The error, and the line number it names.
    ///
    /// Both matter. A reader that reports the wrong line sends whoever has to
    /// repair the file to the wrong place, and every case here is one byte away
    /// from a record that is silently short — so an error raised for some other
    /// reason would look like a pass.
    fn refusal(err: impl std::fmt::Display, line: usize) -> String {
        let text = err.to_string();
        assert!(
            text.contains(&format!("at line {line}")),
            "expected the error to point at line {line}: {text}"
        );
        text
    }

    // A record loses one byte — the `\n` at the end of a line arrives as a
    // space, a NUL, a comma — and the line under it is read as part of the line
    // above. Whatever that line held is gone: a move, the whole move list, a
    // branch. The record comes back `Ok` all the same, and no caller can tell it
    // from a shorter game.
    //
    // The move counts here are the point: each case has to be an error, not a
    // record one ply short.
    #[test]
    fn a_line_that_lost_its_ending_is_an_error_not_a_shorter_record() {
        const KIF: &str = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)   ( 0:01/00:00:01)
   2 ３四歩(33)   ( 0:01/00:00:02)
   3 ２二角成(88)   ( 0:01/00:00:03)
";
        assert_eq!(3, parse_kif_str(KIF).expect("parses").moves.len() - 1);
        // A move line joined to the one below it. The byte in the newline's
        // place can be anything, so the shapes are looked for past it too.
        for byte in [" ", "\u{0}", ",", "x"] {
            let joined = KIF.replace("00:00:02)\n", &format!("00:00:02){byte}"));
            let err = parse_kif_str(&joined).expect_err("the joined line is an error");
            refusal(err, 4);
        }

        // A `変化：N手` header joined to the first move of its branch. The whole
        // branch goes missing.
        let with_branch = format!("{KIF}\n変化：3手\n   3 ８八銀(79)   ( 0:01/00:00:03)\n");
        // R-JKF-004: the branch is the alternative *to* ply 3, so it hangs off
        // that node — `moves[3]`, the initial position's slot being `moves[0]`.
        assert!(parse_kif_str(&with_branch).expect("parses").moves[3]
            .forks
            .is_some());
        let err = parse_kif_str(&with_branch.replace("変化：3手\n", "変化：3手 "))
            .expect_err("the joined header is an error");
        refusal(err, 7);

        // KI2: the starting position joined to the moves. Every move is read as
        // part of the header value, and the record comes back with none.
        const KI2: &str = "手合割：平手\n▲７六歩 △３四歩 ▲２二角成\n";
        assert_eq!(3, parse_ki2_str(KI2).expect("parses").moves.len() - 1);
        let err = parse_ki2_str(&KI2.replace("平手\n", "平手 ")).expect_err("an error");
        refusal(err, 1);

        // KI2 again, at the branch header. What follows it need not be a move:
        // an outcome line joined to the header takes the block with it.
        for tail in ["△８四歩", "まで1手で中断"] {
            let joined = format!("手合割：平手\n▲７六歩 △３四歩\n\n変化：2手 {tail}\n");
            let err = parse_ki2_str(&joined).expect_err("the joined header is an error");
            refusal(err, 4);
        }
    }

    // The header block is the same in both formats, but what a header value can
    // have swallowed is not: only KI2 keeps its moves on a line the block could
    // run into. A KIF header naming an opening is made of the same characters
    // and has nothing wrong with it, and so is a KI2 one — a run is two moves.
    #[test]
    fn a_header_that_names_moves_is_only_suspect_as_a_run_in_ki2() {
        for header in ["戦型：▲２六歩から", "消費時間：104▲379△380"] {
            let kif = format!(
                "手合割：平手\n{header}\n手数----指手---------消費時間--\n   1 ７六歩(77)\n"
            );
            assert_eq!(
                1,
                parse_kif_str(&kif).expect("parses").moves.len() - 1,
                "{header}"
            );
            let ki2 = format!("手合割：平手\n{header}\n▲７六歩 △３四歩\n");
            assert_eq!(
                2,
                parse_ki2_str(&ki2).expect("parses").moves.len() - 1,
                "{header}"
            );
        }
        // The run is what a KI2 record whose starting position lost its newline
        // leaves in the value.
        let err = parse_ki2_str("手合割：平手 ▲７六歩 △３四歩\n").expect_err("an error");
        refusal(err, 1);
    }

    // A `変化：N手` says a branch follows it. A file that ends right after one
    // was cut short, and coming back with one fewer branch says nothing about
    // that.
    //
    // Whether a blank line happens to sit in front of the header is not a
    // difference the answer can turn on, so both spellings are here: a run that
    // swallows the header on its way past leaves nothing to ask about, and the
    // branch goes missing quietly in exactly that case.
    #[test]
    fn a_branch_header_with_nothing_under_it_is_an_error() {
        const KIF: &str = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
";
        for (gap, line) in [("\n", 6), ("", 5)] {
            let err = parse_kif_str(&format!("{KIF}{gap}変化：2手\n")).expect_err("an error");
            let text = refusal(err, line);
            assert!(
                text.contains("変化"),
                "the error points at the header itself: {text}"
            );
        }

        // A block whose first line cannot be read is a different fault with a
        // different cause. D1's leftover-input check names that line, and saying
        // "no moves under it" instead would name a cause that is not the one —
        // `パス` is a word this reader has no meaning for (D8), not a missing
        // branch.
        let err = parse_kif_str(&format!("{KIF}\n変化：2手\n   2 パス\n")).expect_err("an error");
        let text = refusal(err, 7);
        assert!(text.contains("パス"), "the error names the word: {text}");
    }

    // Kifu for Windows marks some moves with a trailing `+`
    // (`data/tests/kif/everyday_20211107.kif`). It is not in R-KIF-005's
    // grammar and there is nothing to read it into, but dropping it loses
    // nothing the record was made of — unlike the line underneath.
    #[test]
    fn an_annotation_after_a_move_is_still_skipped() {
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)   ( 0:01/00:00:01)+
   2 ３四歩(33)   ( 0:01/00:00:02)!?
";
        assert_eq!(2, parse_kif_str(kif).expect("parses").moves.len() - 1);
    }

    // Leftover input is not enough on its own. A line the move list has no
    // shape for is skipped whole, so an input made only of such lines leaves
    // nothing behind to report and used to come back as an empty record.
    //
    // The consumer decides a text encoding on this answer: obs-shogi tries
    // encodings in turn and takes the first `Ok`. `bug_big.kif` is meant to be
    // rejected at its unreadable line (D1/D8) — instead the UTF-16LE attempt
    // decoded it to one long line of mojibake, that came back `Ok` with zero
    // moves, and *that* won. The error D1 exists to raise reached nobody.
    #[test]
    fn a_file_that_is_not_a_kifu_is_an_error_not_an_empty_record() {
        for src in [
            // No newline at all: one line the reader has no shape for.
            "これは棋譜ではないただの塊",
            "これは棋譜ではない\nただの文章だ\n",
            // A `.kif` holding something else entirely.
            "{\"header\":{},\"moves\":[{}]}\n",
            "V2.2\nPI\n+\n+7776FU\n",
        ] {
            assert!(parse_kif_str(src).is_err(), "{src:?} came back as a record");
            assert!(parse_ki2_str(src).is_err(), "{src:?} came back as a record");
        }
    }

    // A record can legitimately hold no moves — a header the reader understood
    // is enough to say the file is a kifu. `手合割：平手` and a file that is not
    // a kifu produce the same `initial`, so only the reader can tell them
    // apart, and it does that by whether it consumed a header line.
    #[test]
    fn a_kifu_with_nothing_but_a_header_is_still_a_kifu() {
        for src in [
            "手合割：平手\n",
            "手合割：平手\n手数----指手---------消費時間--\n",
            "先手：Aさん\n",
            "*ひとこと\n",
            "",
            "  \n\n",
        ] {
            let jkf = parse_kif_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
            assert_eq!(0, jkf.moves.len() - 1, "{src:?}");
        }
    }

    // The other side of the same check: the lines a record legitimately ends
    // with are accounted for, so a whole file does not come back as an error.
    // `まで<N>手で<結末>` is one of them, and it need not have a newline after it.
    #[test]
    fn the_lines_a_record_ends_with_are_not_leftover_input() {
        const RECORD: &str =
            "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ３四歩(33)\n";
        for tail in [
            "",
            "まで2手で中断\n",
            "まで2手で中断",
            "\n\n",
            "変化：2\n   2 ８四歩(83)\n",
            // More than one trailing line. The run parser already swallows the
            // first, so stopping there would accept one and call the second
            // unreadable.
            "まで2手で中断\n感想：良い将棋\n",
            "解説A\n解説B\n",
        ] {
            let jkf = parse_kif_str(&format!("{RECORD}{tail}"))
                .unwrap_or_else(|e| panic!("{tail:?} was rejected: {e}"));
            assert_eq!(2, jkf.moves.len() - 1, "{tail:?}");
        }
    }

    // KI2 reads a whole line at a time, so a line it cannot spell out is the
    // same silent truncation in a different shape.
    //
    // A line with no move on it is a different matter: D10 skips those, the
    // same way KIF always has. `ki2_skips_the_same_lines_kif_does_and_no_more`
    // draws that boundary.
    #[test]
    fn a_ki2_record_the_reader_stops_in_the_middle_of_is_an_error() {
        for (src, word) in [
            // Mid-line: the rest of the line holds a move, so skipping the line
            // would take that move with it.
            ("手合割：平手\n▲７六歩 ほげ △三四歩\n", "ほげ"),
            ("手合割：平手\n▲７六歩\nほげほげ △３四歩\n", "ほげほげ"),
            // A KIF move line in a KI2.
            ("手合割：平手\n▲７六歩\n   2 ３四歩(33)\n", "３四歩(33)"),
        ] {
            let err = parse_ki2_str(src).expect_err("{word} should stop the reader");
            assert!(
                err.to_string().contains(word),
                "{word} is missing from {err}"
            );
        }
    }

    use super::*;
    use crate::jkf::*;
    use serde_json::Value;
    use std::ffi::OsStr;
    use std::io::Result;

    /// Writes `bytes` to a scratch file named `name` and hands back the path.
    ///
    /// The extension is the input here — `parse_kif_file` and `parse_ki2_file`
    /// pick the encoding from it — so these cases cannot be expressed as strings
    /// and there is no fixture on disk with the UTF-8 extensions.
    fn scratch(name: &str, bytes: &[u8]) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join("shogi_kifu_converter_tests");
        std::fs::create_dir_all(&dir).expect("creates the scratch directory");
        let path = dir.join(name);
        std::fs::write(&path, bytes).expect("writes the scratch file");
        path
    }

    // Which encoding is tried *first*, for bytes both of them read cleanly.
    // `竜王戦` in UTF-8 is also a run of valid Shift-JIS, so the same file has
    // two readings and the extension picks between them. Nothing else in the
    // suite pins the order: swapping the two arms of `decode_kifu` leaves every
    // other test green, and the choice is visible to the consumer — the same
    // bytes saved as `.kif` and as `.kifu` come back as different text.
    #[test]
    fn the_extension_decides_which_of_two_readings_wins() {
        // `竜王戦` in UTF-8 is also a run of valid Shift-JIS, so these bytes
        // have two clean readings and the first one asked for wins.
        let both_ways = "竜王戦".as_bytes();
        assert_eq!(
            "竜王戦",
            decode_kifu(both_ways, UTF_8).expect("reads as UTF-8")
        );
        assert_eq!(
            "遶懃視謌ｦ",
            decode_kifu(both_ways, SHIFT_JIS).expect("reads as Shift-JIS"),
            "the Shift-JIS arm returned the UTF-8 reading"
        );
        // And the fallback still runs when the first reading is not clean: a
        // `.kif` holding UTF-8 is what `either_encoding_is_read_whatever_the_extension_says`
        // covers end to end.
        assert_eq!(
            "手合割：平手\n",
            decode_kifu("手合割：平手\n".as_bytes(), SHIFT_JIS).expect("falls back")
        );
    }

    // `.kifu` and `.ki2u` are the UTF-8 spellings of `.kif` and `.ki2`. The
    // extension chooses which encoding is *tried first* — the bytes decide the
    // rest (R-REQ-003 / D14, and `either_encoding_is_read_whatever_the_extension_says`
    // below) — but it is the extension alone that decides whether a file is
    // offered at all. Nothing under `data/tests/` carries either UTF-8 spelling,
    // so that arm went unrun, as did the rejection of an extension neither
    // reader claims.
    #[test]
    fn the_extension_chooses_which_encoding_is_tried_first() {
        const KIF: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
        const KI2: &str = "手合割：平手\n▲７六歩\n";
        let sjis = |s: &str| SHIFT_JIS.encode(s).0.into_owned();

        let from_kif = parse_kif_file(scratch("utf8.kifu", KIF.as_bytes())).expect("reads .kifu");
        assert_eq!(
            from_kif,
            parse_kif_file(scratch("sjis.kif", &sjis(KIF))).expect("reads .kif")
        );
        let from_ki2 = parse_ki2_file(scratch("utf8.ki2u", KI2.as_bytes())).expect("reads .ki2u");
        assert_eq!(
            from_ki2,
            parse_ki2_file(scratch("sjis.ki2", &sjis(KI2))).expect("reads .ki2")
        );

        // Windows writes `.KIF` as readily as `.kif`, and the extension is only
        // a hint at the encoding to begin with — so it is matched case-folded.
        for name in ["upper.KIF", "mixed.Kif"] {
            assert!(
                parse_kif_file(scratch(name, &sjis(KIF))).is_ok(),
                "{name} was rejected"
            );
        }
        for name in ["upper.KI2", "mixed.Ki2"] {
            assert!(
                parse_ki2_file(scratch(name, &sjis(KI2))).is_ok(),
                "{name} was rejected"
            );
        }

        for path in [
            scratch("kifu.txt", KIF.as_bytes()),
            scratch("noextension", KIF.as_bytes()),
        ] {
            assert!(matches!(
                parse_kif_file(&path),
                Err(ParseError::FileExtension)
            ));
            assert!(matches!(
                parse_ki2_file(&path),
                Err(ParseError::FileExtension)
            ));
        }
    }

    // R-REQ-003: the extension names an encoding but does not guarantee one, so
    // every extension has to read both. All four combinations turn up — a `.kif`
    // holding UTF-8 above all — and three of the four used to come back as
    // `Decode Error`.
    //
    // The decision belongs at the decode, not at a failed parse. The full-width
    // forms a kifu is written in (`７`, `：`) sit at U+FF00 and up, whose UTF-8
    // lead byte is `0xEF`, and Shift-JIS has nothing there — so a UTF-8 file
    // read as Shift-JIS fails while decoding and never reaches a parse error to
    // retry on. A retry hung off `ParseError::Kif` is unreachable.
    #[test]
    fn either_encoding_is_read_whatever_the_extension_says() {
        const KIF: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
        const KI2: &str = "手合割：平手\n▲７六歩 △３四歩\n";
        let sjis = |s: &str| SHIFT_JIS.encode(s).0.into_owned();

        for (name, bytes) in [
            ("both.kif", sjis(KIF)),
            ("both_utf8.kif", KIF.as_bytes().to_vec()),
            ("both.kifu", KIF.as_bytes().to_vec()),
            ("both_sjis.kifu", sjis(KIF)),
        ] {
            let jkf = parse_kif_file(scratch(name, &bytes))
                .unwrap_or_else(|e| panic!("{name} was not read: {e}"));
            assert_eq!(1, jkf.moves.len() - 1, "{name}");
        }
        for (name, bytes) in [
            ("both.ki2", sjis(KI2)),
            ("both_utf8.ki2", KI2.as_bytes().to_vec()),
            ("both.ki2u", KI2.as_bytes().to_vec()),
            ("both_sjis.ki2u", sjis(KI2)),
        ] {
            let jkf = parse_ki2_file(scratch(name, &bytes))
                .unwrap_or_else(|e| panic!("{name} was not read: {e}"));
            assert_eq!(2, jkf.moves.len() - 1, "{name}");
        }
    }

    // R-CSA-001: the CSA spec leaves the encoding to whatever wrote the file, so
    // the bytes are all there is to go on — and the Windows GUIs that write most
    // CSA in the wild write Shift-JIS. Reading the file as UTF-8 and nothing
    // else makes the same game readable or not depending on the format it was
    // saved in, while KIF and KI2 next to it read both.
    #[test]
    fn a_csa_is_read_in_either_encoding() {
        const CSA: &str = "V2.2\nN+山田太郎\nN-田中一郎\nPI\n+\n+7776FU\n";
        let sjis = SHIFT_JIS.encode(CSA).0.into_owned();

        let from_utf8 = parse_csa_file(scratch("both.csa", CSA.as_bytes())).expect("reads UTF-8");
        let from_sjis = parse_csa_file(scratch("sjis.csa", &sjis)).expect("reads Shift-JIS");
        assert_eq!(from_utf8, from_sjis);
        assert_eq!(1, from_utf8.moves.len() - 1);

        // The extension is only ever a hint at an encoding (`read_kifu`), and
        // R-CSA-001 leaves the encoding to whatever wrote the file — so there is
        // nothing for an extension to say, and none is refused.
        for name in ["upper.CSA", "named.txt", "noextension"] {
            assert!(
                parse_csa_file(scratch(name, &sjis)).is_ok(),
                "{name} was rejected"
            );
        }
    }

    #[test]
    fn csa_to_jkf() -> Result<()> {
        let dir = Path::new("data/tests/csa");
        for entry in dir.read_dir()? {
            // Parse and convert CSA to JKF
            let mut path = entry?.path();
            if path.extension() != Some(OsStr::new("csa")) {
                continue;
            }
            let jkf = match parse_csa_file(&path) {
                Ok(jkf) => jkf,
                Err(err) => panic!("failed to parse csa {}: {err}", path.display()),
            };
            // Load exptected JSON
            assert!(path.set_extension("json"));
            let file = File::open(&path)?;
            let mut expected = serde_json::from_reader::<_, JsonKifuFormat>(BufReader::new(file))
                .expect("failed to parse json");
            // Remove all move comments (they cannot be restored from csa...)
            expected.moves.iter_mut().for_each(|m| m.comments = None);

            assert_eq!(expected, jkf, "different from expected: {}", path.display());
        }
        Ok(())
    }

    #[test]
    fn kif_to_jkf() -> Result<()> {
        let dir = Path::new("data/tests/kif");
        for entry in dir.read_dir()? {
            // Parse and convert KIF to JKF, and serialize to Value
            let mut path = entry?.path();
            if path.extension() != Some(OsStr::new("kif")) {
                continue;
            }
            let mut jkf = match parse_kif_file(&path) {
                Ok(jkf) => jkf,
                Err(err) => {
                    panic!("failed to parse kif file {}: {err}", path.display());
                }
            };
            // KIF fast-path skips relative inference; populate to match golden JSON
            jkf.populate_relative().expect("populate_relative failed");
            let val = serde_json::to_value(&jkf).expect("failed to serialize jkf");
            // Load exptected JSON Value
            assert!(path.set_extension("json"));
            let file = File::open(&path)?;
            let expected = serde_json::from_reader::<_, Value>(BufReader::new(file))
                .expect("failed to parse json");

            assert_eq!(expected, val, "different from expected: {}", path.display());
        }
        Ok(())
    }

    #[test]
    fn ki2_to_jkf() -> Result<()> {
        let dir = Path::new("data/tests/ki2");
        for entry in dir.read_dir()? {
            // Parse and convert KI2 to JKF, and serialize to Value
            let mut path = entry?.path();
            if path.extension() != Some(OsStr::new("ki2")) {
                continue;
            }
            let jkf = match parse_ki2_file(&path) {
                Ok(jkf) => jkf,
                Err(err) => {
                    panic!("failed to parse ki2 file {}: {err}", path.display());
                }
            };
            let val = serde_json::to_value(&jkf).expect("failed to serialize jkf");
            // Load exptected JSON Value
            assert!(path.set_extension("json"));
            let file = File::open(&path)?;
            let expected = serde_json::from_reader::<_, Value>(BufReader::new(file))
                .expect("failed to parse json");

            assert_eq!(expected, val, "different from expected: {}", path.display());
        }
        Ok(())
    }
}
