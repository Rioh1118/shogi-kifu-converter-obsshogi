//! The words each format uses for a [`MoveSpecial`].
//!
//! The data model says what happened; how KIF, KI2 and CSA spell it is a
//! property of the format, and the reader and writer of each format have to
//! agree on it. These are the only mappings between [`MoveSpecial`] and the
//! three vocabularies, so that the two directions cannot drift apart.

use crate::jkf::{Color, MoveSpecial};

impl MoveSpecial {
    /// The KIF word for this special move, or `None` when KIF has no word for it.
    ///
    /// This and [`Self::from_kif_word`] are the only mapping between
    /// [`MoveSpecial`] and the KIF vocabulary. Every reader and writer goes
    /// through them, so that the two directions cannot disagree. The KIF
    /// shared reader's `KIF_SPECIAL_WORDS` lists which of them it looks for, and
    /// a word added here has to be added there too or it is never read — which
    /// is what `MoveSpecial::ALL` and the test that walks it are for.
    ///
    /// R-KIF-007 has no word for `HIKIWAKE`, `MATTA` or `ERROR`. Where they
    /// land is D7's decision, not a rule: `HIKIWAKE` to 持将棋 so that the game
    /// still reads as drawn, the other two to nothing.
    pub(crate) fn kif_word(self) -> Option<&'static str> {
        Some(match self {
            MoveSpecial::SpecialToryo => "投了",
            MoveSpecial::SpecialChudan => "中断",
            MoveSpecial::SpecialSennichite => "千日手",
            MoveSpecial::SpecialTimeUp => "切れ負け",
            MoveSpecial::SpecialIllegalMove => "反則負け",
            MoveSpecial::SpecialIllegalActionBlack | MoveSpecial::SpecialIllegalActionWhite => {
                "反則勝ち"
            }
            MoveSpecial::SpecialJishogi | MoveSpecial::SpecialHikiwake => "持将棋",
            MoveSpecial::SpecialKachi => "入玉勝ち",
            MoveSpecial::SpecialTsumi => "詰み",
            MoveSpecial::SpecialFuzumi => "不詰",
            // R-KIF-007 names these against 先手, not against whoever is to
            // move, so there is nothing to derive and no pair to swap — unlike
            // 反則勝ち, which the same table defines relative to the move before
            // it. (The spec glosses 先手 here as 上手, against its own ▲＝下手;
            // see `research/30-kif.md`.)
            MoveSpecial::SpecialFusensho => "不戦勝",
            MoveSpecial::SpecialFusenpai => "不戦敗",
            MoveSpecial::SpecialMatta | MoveSpecial::SpecialError => return None,
        })
    }

    /// Parses a KIF special move word.
    ///
    /// `side_to_move` is whose turn it is at that ply. 反則勝ち means the move
    /// *before* it was the foul (R-KIF-007), so the offender is the other
    /// player, and that is what picks between `+ILLEGAL_ACTION` and
    /// `-ILLEGAL_ACTION`.
    ///
    /// 不戦勝 and 不戦敗 have no JKF counterpart, so `SpecialFusensho` and
    /// `SpecialFusenpai` are extensions to the standard's fourteen (D1). The
    /// JSON this crate writes for them is not readable by json-kifu-format —
    /// `research/33-jkf.md` says so. Leaving them out instead is worse: they
    /// are R-KIF-007 vocabulary, and a reader that does not know them refuses
    /// a valid KIF as a whole (D1).
    pub(crate) fn from_kif_word(word: &str, side_to_move: Color) -> Option<Self> {
        Some(match word {
            "投了" => MoveSpecial::SpecialToryo,
            "中断" => MoveSpecial::SpecialChudan,
            "千日手" => MoveSpecial::SpecialSennichite,
            "切れ負け" => MoveSpecial::SpecialTimeUp,
            "反則負け" => MoveSpecial::SpecialIllegalMove,
            "反則勝ち" => match side_to_move {
                Color::Black => MoveSpecial::SpecialIllegalActionWhite,
                Color::White => MoveSpecial::SpecialIllegalActionBlack,
            },
            "持将棋" => MoveSpecial::SpecialJishogi,
            "入玉勝ち" => MoveSpecial::SpecialKachi,
            "詰み" => MoveSpecial::SpecialTsumi,
            "不詰" => MoveSpecial::SpecialFuzumi,
            "不戦勝" => MoveSpecial::SpecialFusensho,
            "不戦敗" => MoveSpecial::SpecialFusenpai,
            _ => return None,
        })
    }

    /// The KI2 end-of-game phrase — the part after `まで<N>手で`.
    ///
    /// The spellings are D5's. R-KI2-006 is why there is a decision to make —
    /// the published specification says nothing about how a KI2 game ends — and
    /// D5 is where following tsshogi was chosen, because that is the library
    /// reading these same files on the TypeScript side of the only consumer.
    ///
    /// `side_to_move` is whose turn it is at the ply the special move occupies.
    pub(crate) fn ki2_phrase(self, side_to_move: Color) -> String {
        // `mover` is the side to move; `opponent` is the one who just moved.
        let (mover, opponent) = match side_to_move {
            Color::Black => ("先手", "後手"),
            Color::White => ("後手", "先手"),
        };
        match self {
            MoveSpecial::SpecialToryo => format!("{opponent}の勝ち"),
            MoveSpecial::SpecialTimeUp => format!("時間切れにより{opponent}の勝ち"),
            MoveSpecial::SpecialKachi => format!("{mover}の入玉勝ち"),
            // The phrase names the winner outright, and the variant already
            // says who it is: `+ILLEGAL_ACTION` is a foul *by* Black, so White
            // wins (R-CSA-007). Deriving it from the side to move instead makes
            // the two sides swap places whenever that side is computed wrong.
            MoveSpecial::SpecialIllegalActionBlack => "後手の反則勝ち".to_owned(),
            MoveSpecial::SpecialIllegalActionWhite => "先手の反則勝ち".to_owned(),
            MoveSpecial::SpecialIllegalMove => format!("{mover}の反則負け"),
            _ => self.kif_word().unwrap_or("中断").to_owned(),
        }
    }

    /// Parses a KI2 end-of-game phrase, with `まで<N>手で` already removed.
    ///
    /// The order of the tests matters: `時間切れにより…の勝ち`, `反則勝ち` and
    /// `入玉勝ち` all end in `勝ち`, so the plain resignation case comes last.
    pub(crate) fn from_ki2_phrase(phrase: &str, side_to_move: Color) -> Option<Self> {
        if phrase.starts_with("時間切れ") {
            return Some(MoveSpecial::SpecialTimeUp);
        }
        if phrase.ends_with("反則勝ち") {
            // Unlike KIF, the KI2 phrase names the winner, so the side to move
            // is only a fallback for a spelling that does not name one.
            return match phrase.strip_suffix("の反則勝ち") {
                Some("先手") => Some(MoveSpecial::SpecialIllegalActionWhite),
                Some("後手") => Some(MoveSpecial::SpecialIllegalActionBlack),
                _ => MoveSpecial::from_kif_word("反則勝ち", side_to_move),
            };
        }
        if phrase.ends_with("反則負け") {
            return Some(MoveSpecial::SpecialIllegalMove);
        }
        if phrase.ends_with("入玉勝ち") {
            return Some(MoveSpecial::SpecialKachi);
        }
        if phrase.ends_with("勝ち") {
            return Some(MoveSpecial::SpecialToryo);
        }
        MoveSpecial::from_kif_word(phrase, side_to_move)
    }

    /// The CSA `%` keyword for this special move, without the leading `%`.
    pub(crate) fn csa_word(self) -> &'static str {
        match self {
            // CSA's keyword list is the 14 this enum started from (R-CSA-007),
            // so the two KIF adds have none. `%CHUDAN` — the game stopped here
            // — is the most that can be said without asserting something CSA
            // does not mean; calling a no-show a foul would be worse than
            // losing the distinction (D11, the same trade as D7).
            MoveSpecial::SpecialFusensho | MoveSpecial::SpecialFusenpai => "CHUDAN",
            MoveSpecial::SpecialToryo => "TORYO",
            MoveSpecial::SpecialChudan => "CHUDAN",
            MoveSpecial::SpecialSennichite => "SENNICHITE",
            MoveSpecial::SpecialTimeUp => "TIME_UP",
            MoveSpecial::SpecialIllegalMove => "ILLEGAL_MOVE",
            MoveSpecial::SpecialIllegalActionBlack => "+ILLEGAL_ACTION",
            MoveSpecial::SpecialIllegalActionWhite => "-ILLEGAL_ACTION",
            MoveSpecial::SpecialJishogi => "JISHOGI",
            MoveSpecial::SpecialKachi => "KACHI",
            MoveSpecial::SpecialHikiwake => "HIKIWAKE",
            MoveSpecial::SpecialMatta => "MATTA",
            MoveSpecial::SpecialTsumi => "TSUMI",
            MoveSpecial::SpecialFuzumi => "FUZUMI",
            MoveSpecial::SpecialError => "ERROR",
        }
    }
}
