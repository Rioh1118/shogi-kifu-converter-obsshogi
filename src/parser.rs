//! Parsers for [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)

mod kakinoki;
mod ki2;
mod kif;

use crate::error::ParseError;
use crate::jkf::JsonKifuFormat;
use encoding_rs::{SHIFT_JIS, UTF_8};
use nom::error::convert_error;
use nom::Finish;
use std::fs::File;
use std::io::{BufReader, Read};
use std::path::Path;

/// Parses a CSA file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the file.
pub fn parse_csa_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let mut file = File::open(&path)?;
    let mut buf = String::new();
    file.read_to_string(&mut buf)?;
    parse_csa_str(&buf)
}

/// Parses a CSA formatted string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the string.
pub fn parse_csa_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let mut jkf = JsonKifuFormat::try_from(csa::parse_csa(s)?)?;
    jkf.normalize()?;
    Ok(jkf)
}

/// Parses a KIF file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// If the file extension is `.kif`, it is decoded as Shift-JIS, and if it is `.kifu`, it is decoded as UTF-8 and parsed.
///
/// See: [http://kakinoki.o.oo7.jp/kif_format.html](http://kakinoki.o.oo7.jp/kif_format.html)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the file.
pub fn parse_kif_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let mut file = File::open(&path)?;
    let ext = path.as_ref().extension().ok_or(ParseError::FileExtension)?;
    let encoding = match ext.to_str() {
        Some("kif") => SHIFT_JIS,
        Some("kifu") => UTF_8,
        _ => return Err(ParseError::FileExtension),
    };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let (cow, _, had_errors) = encoding.decode(&buf);
    if had_errors {
        // Decoding failed outright, try UTF-8
        let (cow_utf8, _, had_errors_utf8) = UTF_8.decode(&buf);
        if had_errors_utf8 {
            return Err(ParseError::Decode);
        }
        return parse_kif_str(&cow_utf8);
    }
    // Decoding succeeded, but the content may be garbled (e.g. UTF-8 file decoded as Shift-JIS).
    // Only retry on parse-grammar errors (ParseError::Kif) — for Normalize errors, retrying
    // with another encoding produces the same error and doubles the cost of a 4 s parse.
    match parse_kif_str(&cow) {
        Ok(jkf) => Ok(jkf),
        Err(err @ ParseError::Kif(_)) if encoding == SHIFT_JIS => {
            let (cow_utf8, _, had_errors_utf8) = UTF_8.decode(&buf);
            if had_errors_utf8 {
                return Err(err);
            }
            parse_kif_str(&cow_utf8).or(Err(err))
        }
        Err(err) => Err(err),
    }
}

/// Whether the reader got no move out of `jkf` and left `rest` behind.
///
/// Every reader here returns an empty record rather than an error for input it
/// recognised no part of. That is a silent failure on its own, and it becomes a
/// loud one in the consumer: obs-shogi tries encodings in turn and takes the
/// first `Ok`, so a mojibake decode that yields an empty record wins over the
/// decode that would have read the game (D1).
///
/// This is narrower than the strictness D1 asks for — a record whose *tail* is
/// unreadable still comes back truncated (GAP-005) — but it separates "read
/// nothing" from "read a record that has no moves", which a header-only file
/// legitimately is.
fn read_nothing(jkf: &JsonKifuFormat, rest: &str) -> bool {
    !rest.trim().is_empty()
        && jkf
            .moves
            .iter()
            .all(|mf| mf.move_.is_none() && mf.special.is_none())
}

/// Parses a KIF formatted string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the string.
pub fn parse_kif_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    match kif::parse(s).finish() {
        Ok((rest, mut jkf)) => {
            if read_nothing(&jkf, rest) {
                return Err(ParseError::Kif(convert_error(
                    s,
                    nom::error::VerboseError {
                        errors: vec![(rest, nom::error::VerboseErrorKind::Context("no KIF here"))],
                    },
                )));
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
/// If the file extension is `.ki2`, it is decoded as Shift-JIS, and if it is `.ki2u`, it is decoded as UTF-8 and parsed.
///
/// See: [http://kakinoki.o.oo7.jp/KifuwInt.htm](http://kakinoki.o.oo7.jp/KifuwInt.htm)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the file.
pub fn parse_ki2_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let mut file = File::open(&path)?;
    let ext = path.as_ref().extension().ok_or(ParseError::FileExtension)?;
    let encoding = match ext.to_str() {
        Some("ki2") => SHIFT_JIS,
        Some("ki2u") => UTF_8,
        _ => return Err(ParseError::FileExtension),
    };
    let mut buf = Vec::new();
    file.read_to_end(&mut buf)?;
    let (cow, _, had_errors) = encoding.decode(&buf);
    if had_errors {
        return Err(ParseError::Decode);
    }
    parse_ki2_str(&cow)
}

/// Parses a KI2 formatted string to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the string.
pub fn parse_ki2_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    match ki2::parse(s).finish() {
        Ok((rest, mut jkf)) => {
            if read_nothing(&jkf, rest) {
                return Err(ParseError::Ki2(convert_error(
                    s,
                    nom::error::VerboseError {
                        errors: vec![(rest, nom::error::VerboseErrorKind::Context("no KI2 here"))],
                    },
                )));
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
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the file.
pub fn parse_jkf_file<P: AsRef<Path>>(path: P) -> Result<JsonKifuFormat, ParseError> {
    let file = File::open(&path)?;
    let mut jkf = serde_json::from_reader::<_, JsonKifuFormat>(BufReader::new(file))?;
    jkf.normalize()?;
    Ok(jkf)
}

/// Parses a JSON file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the file.
pub fn parse_jkf_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let mut jkf = serde_json::from_str::<JsonKifuFormat>(s)?;
    jkf.normalize()?;
    Ok(jkf)
}

#[cfg(test)]
mod tests {
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

    // `.kifu` and `.ki2u` are the UTF-8 spellings of `.kif` and `.ki2`. Both
    // readers dispatch on the extension alone, and nothing under `data/tests/`
    // carries either one, so the whole UTF-8 arm went unrun — as did the
    // rejection of an extension neither arm claims.
    #[test]
    fn the_extension_picks_the_encoding() {
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

    // A `.kif` holding UTF-8 is common enough that the reader falls back rather
    // than failing. Which fallback carries it matters: the full-width forms a
    // KIF is written in (`７`, `：`) sit at U+FF00 and up, whose UTF-8 lead byte
    // is `0xEF`, and no Shift-JIS character starts there — so the *decode*
    // reports an error and the UTF-8 retry happens before any parsing.
    #[test]
    fn a_utf8_file_named_kif_is_still_read() {
        const KIF: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
        let mislabelled = parse_kif_file(scratch("utf8.kif", KIF.as_bytes())).expect("reads");
        assert_eq!(1, mislabelled.moves.len() - 1);
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
