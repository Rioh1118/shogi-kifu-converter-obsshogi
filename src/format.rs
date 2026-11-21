//! The text formats this crate reads and writes, named by one value.

/// A kifu format this crate can both read and write.
///
/// One value instead of one function per format, so that a caller choosing by
/// file extension writes the choice once: [`Format::from_extension`] to pick,
/// [`crate::parser::parse_str`] to read and
/// [`JsonKifuFormat::try_to_string`](crate::jkf::JsonKifuFormat::try_to_string)
/// to write. USI is not here: it is written but never read.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Format {
    /// KIF (R-KIF-001): `.kif` (Shift_JIS) or `.kifu` (UTF-8).
    Kif,
    /// KI2: `.ki2` (Shift_JIS) or `.ki2u` (UTF-8).
    Ki2,
    /// CSA (R-CSA-001): `.csa`.
    Csa,
    /// JSON Kifu Format: `.jkf` or `.json`, always UTF-8.
    Jkf,
}

impl Format {
    /// Every format, so that a table over them can be checked for gaps.
    pub const ALL: [Format; 4] = [Format::Kif, Format::Ki2, Format::Csa, Format::Jkf];

    /// The format a file extension names, ignoring case, or `None` for an
    /// extension this crate does not read.
    ///
    /// `ext` is the extension alone, without the dot (`"kif"`, not `".kif"`).
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_ascii_lowercase().as_str() {
            "kif" | "kifu" => Some(Format::Kif),
            "ki2" | "ki2u" => Some(Format::Ki2),
            "csa" => Some(Format::Csa),
            "jkf" | "json" => Some(Format::Jkf),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::jkf::JsonKifuFormat;
    use crate::parser::parse_str;
    use crate::parser::{parse_csa_str, parse_file, parse_jkf_str, parse_ki2_str, parse_kif_str};

    #[test]
    fn every_extension_the_readers_claim_names_its_format_in_any_case() {
        for (ext, want) in [
            ("kif", Format::Kif),
            ("KIF", Format::Kif),
            ("kifu", Format::Kif),
            ("ki2", Format::Ki2),
            ("Ki2u", Format::Ki2),
            ("csa", Format::Csa),
            ("jkf", Format::Jkf),
            ("JSON", Format::Jkf),
        ] {
            assert_eq!(Some(want), Format::from_extension(ext), "{ext}");
        }
        for ext in ["", ".kif", "txt", "usi"] {
            assert_eq!(None, Format::from_extension(ext), "{ext}");
        }
    }

    /// `parse_str` and `try_to_string` are the per-format functions chosen by
    /// a value. A format wired to the wrong function reads as another format's
    /// error, so every format writes and reads back through both paths.
    #[test]
    fn every_format_reads_back_what_it_writes_through_the_format_value() {
        let record = parse_kif_str(
            "手合割：香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n   2 ７六歩(77)\n   3 投了\n",
        )
        .expect("a record to write");
        for format in Format::ALL {
            let text = record
                .try_to_string(format)
                .unwrap_or_else(|e| panic!("{format:?}: {e}"));
            let by_value = parse_str(format, &text).unwrap_or_else(|e| panic!("{format:?}: {e}"));
            let by_name = match format {
                Format::Kif => parse_kif_str(&text),
                Format::Ki2 => parse_ki2_str(&text),
                Format::Csa => parse_csa_str(&text),
                Format::Jkf => parse_jkf_str(&text),
            }
            .unwrap_or_else(|e| panic!("{format:?}: {e}"));
            assert_eq!(by_name, by_value, "{format:?}");
            let moves = |jkf: &JsonKifuFormat| {
                jkf.moves
                    .iter()
                    .filter_map(|mf| mf.move_.map(|m| (m.color, m.to)))
                    .collect::<Vec<_>>()
            };
            assert_eq!(moves(&record), moves(&by_value), "{format:?}: {text}");
        }
    }

    #[test]
    fn parse_file_picks_the_reader_from_the_extension() {
        for (path, format) in [
            ("data/tests/kif/empty.kif", Format::Kif),
            ("data/tests/ki2/4.ki2", Format::Ki2),
            ("data/tests/csa/floodgate.csa", Format::Csa),
        ] {
            let text = std::fs::read(path).expect("sample file");
            let (decoded, _, _) = if format == Format::Csa {
                encoding_rs::UTF_8.decode(&text)
            } else {
                encoding_rs::SHIFT_JIS.decode(&text)
            };
            let by_path = parse_file(path).unwrap_or_else(|e| panic!("{path}: {e}"));
            let by_text = parse_str(format, &decoded).unwrap_or_else(|e| panic!("{path}: {e}"));
            assert_eq!(by_text, by_path, "{path}");
        }
        // `.json` is the JKF the KIF sample was converted to.
        let by_path = parse_file("data/tests/kif/empty.json").expect("a JKF file");
        let text = std::fs::read_to_string("data/tests/kif/empty.json").expect("sample file");
        assert_eq!(parse_jkf_str(&text).expect("a JKF"), by_path);
        assert!(matches!(
            parse_file("Cargo.toml"),
            Err(crate::error::ParseError::FileExtension)
        ));
    }
}
