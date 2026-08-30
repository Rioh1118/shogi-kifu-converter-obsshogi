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
    if let Err(err) = jkf.normalize() {
        Err(ParseError::Normalize(err.to_string()))
    } else {
        Ok(jkf)
    }
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
            if let Err(err) = jkf.normalize_with_options(true, false) {
                Err(ParseError::Normalize(err.to_string()))
            } else {
                Ok(jkf)
            }
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
            if let Err(err) = jkf.normalize_with_color_correction(true) {
                Err(ParseError::Normalize(err.to_string()))
            } else {
                Ok(jkf)
            }
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
    if let Err(err) = jkf.normalize() {
        Err(ParseError::Normalize(err.to_string()))
    } else {
        Ok(jkf)
    }
}

/// Parses a JSON file to [`jkf::JsonKifuFormat`](crate::jkf::JsonKifuFormat)
///
/// # Errors
///
/// This function returns [`ConvertError`](crate::error::ConvertError) if it fails to parse the file.
pub fn parse_jkf_str(s: &str) -> Result<JsonKifuFormat, ParseError> {
    let mut jkf = serde_json::from_str::<JsonKifuFormat>(s)?;
    if let Err(err) = jkf.normalize() {
        Err(ParseError::Normalize(err.to_string()))
    } else {
        Ok(jkf)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::Value;
    use std::ffi::OsStr;
    use std::io::Result;

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
