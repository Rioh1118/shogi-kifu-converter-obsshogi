//! Turning a file into text: which encoding, and what is not part of the record.
//!
//! Every reader takes a `&str`, so this is decided once, before any of them
//! sees the input.

use crate::error::ParseError;
use encoding_rs::{Encoding, SHIFT_JIS, UTF_8};
use std::fs::File;
use std::io::Read;
use std::path::Path;

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
pub(super) fn read_kifu<P: AsRef<Path>>(
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
pub(super) fn decode_kifu(buf: &[u8], first: &'static Encoding) -> Result<String, ParseError> {
    let second = if first == SHIFT_JIS { UTF_8 } else { SHIFT_JIS };
    for encoding in [first, second] {
        let (text, _, had_errors) = encoding.decode(buf);
        if !had_errors {
            return Ok(text.into_owned());
        }
    }
    Err(ParseError::Decode)
}

/// Decodes the bytes of a CSA file: UTF-8, then Shift_JIS, then EUC-JP.
///
/// CSA leaves the encoding to whatever wrote the file (R-CSA-001), and the
/// programs of the 2000s on Unix wrote EUC-JP (Kifu-for-JS's
/// `2005_YAMADA_GPS.csa`; GAP-042). A CSA record is mostly ASCII, so the bytes
/// that tell the three apart are the names and comments, and a wrong decode
/// there reports errors rather than passing.
pub(super) fn decode_csa(buf: &[u8]) -> Result<String, ParseError> {
    decode_kifu(buf, UTF_8).or_else(|_| {
        let (text, _, had_errors) = encoding_rs::EUC_JP.decode(buf);
        if had_errors {
            Err(ParseError::Decode)
        } else {
            Ok(text.into_owned())
        }
    })
}

/// Drops a byte-order mark from the head of a record.
///
/// A BOM is not part of the text, and every reader below takes what it is given
/// literally: left on, it goes into the first line — `\u{feff}手合割：香落ち` is
/// filed under a key nobody wrote, the handicap is lost, and the record reads as
/// 平手 with every side reversed (R-HC-001 / R-RULE-006). The rest of the file
/// reads, so nothing downstream can tell that the first line went
/// (`research/90-gaps.md` GAP-006). Shogidokoro writes one.
pub(super) fn without_a_byte_order_mark(s: &str) -> &str {
    s.strip_prefix('\u{feff}').unwrap_or(s)
}

/// `s` with every line ending spelled `\n`.
///
/// The readers know `\n` and `\r\n`. A file that went through two
/// conversions arrives with `\r\r\n` (Kifu-for-JS's `20081220kyoochi.kif`), and
/// one from an old Mac with a lone `\r`; read literally, the first line never
/// ends and the record is refused at line 1 (GAP-036). A run of `\r` before a
/// `\n` is one line ending, not blank lines, so the line numbers in an error
/// still count the lines a reader sees.
///
/// Borrows when there is no `\r`, which is every file a Unix tool wrote.
pub(super) fn with_plain_line_ends(s: &str) -> std::borrow::Cow<'_, str> {
    if !s.contains('\r') {
        return std::borrow::Cow::Borrowed(s);
    }
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();
    while let Some(c) = chars.next() {
        if c == '\r' {
            while chars.next_if_eq(&'\r').is_some() {}
            chars.next_if_eq(&'\n');
            out.push('\n');
        } else {
            out.push(c);
        }
    }
    std::borrow::Cow::Owned(out)
}
