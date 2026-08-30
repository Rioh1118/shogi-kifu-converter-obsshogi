//! [`JsonKifuFormat`](crate::jkf::JsonKifuFormat) types
//!
//! Reference: [https://apps.81.la/json-kifu-format/docs/modules/Formats.html](https://apps.81.la/json-kifu-format/docs/modules/Formats.html)

use serde::{Deserialize, Serialize};
use serde_repr::{Deserialize_repr, Serialize_repr};
use std::collections::HashMap;

/// A representation of a side-to-move
#[derive(Serialize_repr, Deserialize_repr, Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Color {
    /// 先手
    Black = 0,
    /// 後手
    White = 1,
}

/// A representation of a piece kind
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    /// 歩兵
    FU = 0,
    /// 香車
    KY = 1,
    /// 桂馬
    KE = 2,
    /// 銀将
    GI = 3,
    /// 金将
    KI = 4,
    /// 角行
    KA = 5,
    /// 飛車
    HI = 6,
    /// 玉将
    OU = 7,
    /// と金
    TO = 8,
    /// 成香
    NY = 9,
    /// 成桂
    NK = 10,
    /// 成銀
    NG = 11,
    /// 竜馬
    UM = 12,
    /// 竜王
    RY = 13,
}

/// A representation of a initial state preset
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Preset {
    /// 平手
    #[serde(rename = "HIRATE")]
    PresetHirate,
    /// 香落ち
    #[serde(rename = "KY")]
    PresetKY,
    /// 右香落ち
    #[serde(rename = "KY_R")]
    PresetKYR,
    /// 角落ち
    #[serde(rename = "KA")]
    PresetKA,
    /// 飛車落ち
    #[serde(rename = "HI")]
    PresetHI,
    /// 飛香落ち
    #[serde(rename = "HIKY")]
    PresetHIKY,
    /// 二枚落ち
    #[serde(rename = "2")]
    Preset2,
    /// 三枚落ち
    #[serde(rename = "3")]
    Preset3,
    /// 四枚落ち
    #[serde(rename = "4")]
    Preset4,
    /// 五枚落ち
    #[serde(rename = "5")]
    Preset5,
    /// 左五枚落ち
    #[serde(rename = "5_L")]
    Preset5L,
    /// 六枚落ち
    #[serde(rename = "6")]
    Preset6,
    /// 左七枚落ち
    #[serde(rename = "7_L")]
    Preset7L,
    /// 右七枚落ち
    #[serde(rename = "7_R")]
    Preset7R,
    /// 八枚落ち
    #[serde(rename = "8")]
    Preset8,
    /// 十枚落ち
    #[serde(rename = "10")]
    Preset10,
    /// その他
    #[serde(rename = "OTHER")]
    PresetOther,
}

/// A representation of a relative position information
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum Relative {
    /// 左
    L,
    /// 直
    C,
    /// 右
    R,
    /// 上
    U,
    /// 寄
    M,
    /// 引
    D,
    /// 左上
    LU,
    /// 左寄
    LM,
    /// 左引
    LD,
    /// 右上
    RU,
    /// 右寄
    RM,
    /// 右引
    RD,
    /// 打
    H,
}

/// A representation of a special move
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveSpecial {
    /// 投了
    #[serde(rename = "TORYO")]
    SpecialToryo,
    /// 中断
    #[serde(rename = "CHUDAN")]
    SpecialChudan,
    /// 千日手
    #[serde(rename = "SENNICHITE")]
    SpecialSennichite,
    /// 手番側が時間切れで負け・切れ負け
    #[serde(rename = "TIME_UP")]
    SpecialTimeUp,
    /// 反則負け
    #[serde(rename = "ILLEGAL_MOVE")]
    SpecialIllegalMove,
    /// 先手(下手)の反則行為により、後手(上手)の勝ち
    #[serde(rename = "+ILLEGAL_ACTION")]
    SpecialIllegalActionBlack,
    /// 後手(上手)の反則行為により、先手(下手)の勝ち
    #[serde(rename = "-ILLEGAL_ACTION")]
    SpecialIllegalActionWhite,
    /// 持将棋
    #[serde(rename = "JISHOGI")]
    SpecialJishogi,
    /// (入玉で)勝ちの宣言
    #[serde(rename = "KACHI")]
    SpecialKachi,
    /// (入玉で)引き分けの宣言
    #[serde(rename = "HIKIWAKE")]
    SpecialHikiwake,
    /// 待った
    #[serde(rename = "MATTA")]
    SpecialMatta,
    /// 詰み
    #[serde(rename = "TSUMI")]
    SpecialTsumi,
    /// 不詰
    #[serde(rename = "FUZUMI")]
    SpecialFuzumi,
    /// エラー
    #[serde(rename = "ERROR")]
    SpecialError,
}

impl MoveSpecial {
    /// The KIF word for this outcome, or `None` when KIF has no word for it.
    ///
    /// This and [`Self::from_kif_word`] are the only mapping between
    /// [`MoveSpecial`] and the KIF vocabulary. Every reader and writer goes
    /// through them, so that the two directions cannot disagree. The KIF
    /// parser's `KIF_SPECIAL_WORDS` lists which of them it looks for, and a
    /// word added here has to be added there too or it is never read.
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
            MoveSpecial::SpecialMatta | MoveSpecial::SpecialError => return None,
        })
    }

    /// Parses a KIF outcome word.
    ///
    /// `side_to_move` is whose turn it is at that ply. 反則勝ち means the move
    /// *before* it was the foul (R-KIF-007), so the offender is the other
    /// player, and that is what picks between `+ILLEGAL_ACTION` and
    /// `-ILLEGAL_ACTION`.
    ///
    /// 不戦勝 and 不戦敗 are KIF words with no counterpart here, so they are
    /// not recognised.
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
    /// `side_to_move` is whose turn it is at the ply the outcome occupies.
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

    /// The CSA `%` keyword for this outcome, without the leading `%`.
    pub(crate) fn csa_word(self) -> &'static str {
        match self {
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

/// The type translated from [`IJSONKifuFormat`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.IJSONKifuFormat.html)
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct JsonKifuFormat {
    /// 対局情報
    pub header: HashMap<String, String>,
    /// 開始局面
    pub initial: Option<Initial>,
    /// 指し手
    pub moves: Vec<MoveFormat>,
}

impl Default for JsonKifuFormat {
    fn default() -> Self {
        JsonKifuFormat {
            header: HashMap::new(),
            initial: None,
            moves: vec![MoveFormat::default()],
        }
    }
}

/// The Initial state for [`JsonKifuFormat`]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct Initial {
    /// 手合割
    pub preset: Preset,
    #[serde(skip_serializing_if = "Option::is_none")]
    /// 開始局面情報
    pub data: Option<StateFormat>,
}

/// The type translated from [`IStateFormat`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.IStateFormat.html)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct StateFormat {
    /// 手番
    pub color: Color,
    /// 盤面
    pub board: [[Piece; 9]; 9],
    /// 持ち駒
    pub hands: [Hand; 2],
}

/// The numbers of hand pieces for [`StateFormat::hands`]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[allow(non_snake_case)]
pub struct Hand {
    /// 歩兵
    pub FU: u8,
    /// 香車
    pub KY: u8,
    /// 桂馬
    pub KE: u8,
    /// 銀将
    pub GI: u8,
    /// 金将
    pub KI: u8,
    /// 角行
    pub KA: u8,
    /// 飛車
    pub HI: u8,
}

/// The type translated from [`IPiece`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.IPiece.html)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Piece {
    /// 手番
    #[serde(skip_serializing_if = "Option::is_none")]
    pub color: Option<Color>,
    /// 駒種
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<Kind>,
}

/// The type translated from [`IMoveFormat`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.IMoveFormat.html)
#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct MoveFormat {
    /// 駒移動・駒打ち
    #[serde(rename = "move")]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub move_: Option<MoveMoveFormat>,
    /// コメント
    #[serde(skip_serializing_if = "Option::is_none")]
    pub comments: Option<Vec<String>>,
    /// 消費時間
    #[serde(skip_serializing_if = "Option::is_none")]
    pub time: Option<Time>,
    /// 特殊な指し手(終局・中断など)
    #[serde(skip_serializing_if = "Option::is_none")]
    pub special: Option<MoveSpecial>,
    /// 分岐・変化手順
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forks: Option<Vec<Vec<MoveFormat>>>,
}

impl MoveFormat {
    /// Whether this node gets a ply number of its own.
    ///
    /// A move does, and so does an outcome — KIF numbers `投了` as a line like
    /// any other, and KI2 counts it when it says `まで<N>手で`. A node carrying
    /// only comments or only a time does not: there is nothing in the file for a
    /// reader to number.
    ///
    /// Both writers and the KI2 reader have to agree on this, or a `変化：N手`
    /// block attaches to the wrong move and the outcome names the wrong winner.
    pub(crate) fn occupies_a_ply(&self) -> bool {
        self.move_.is_some() || self.special.is_some()
    }
}

/// The type translated from [`IMoveMoveFormat`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.IMoveMoveFormat.html)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveMoveFormat {
    /// 手番
    pub color: Color,
    /// 移動元
    #[serde(skip_serializing_if = "Option::is_none")]
    pub from: Option<PlaceFormat>,
    /// 移動先
    pub to: PlaceFormat,
    /// 駒種
    pub piece: Kind,
    /// 同
    #[serde(skip_serializing_if = "Option::is_none")]
    pub same: Option<bool>,
    /// 成
    #[serde(skip_serializing_if = "Option::is_none")]
    pub promote: Option<bool>,
    /// 駒取り
    #[serde(skip_serializing_if = "Option::is_none")]
    pub capture: Option<Kind>,
    /// 相対位置・動作
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relative: Option<Relative>,
}

/// The type translated from [`IPlaceFormat`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.IPlaceFormat.html)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaceFormat {
    /// 筋
    pub x: u8,
    /// 段
    pub y: u8,
}

/// The time data for [`MoveFormat::time`]
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Time {
    /// 消費時間
    pub now: TimeFormat,
    /// 累積消費時間
    pub total: TimeFormat,
}

/// The type translated from [`ITimeFormat`](https://apps.81.la/json-kifu-format/docs/interfaces/Formats.ITimeFormat.html)
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TimeFormat {
    /// 時間
    #[serde(skip_serializing_if = "Option::is_none")]
    pub h: Option<u16>,
    /// 分
    ///
    /// Wider than the 0..59 a clock shows, because a KIF states the time spent
    /// on one move as 分:秒 with no limit on the minutes (R-KIF-008): a title
    /// game with nine hours on the clock has moves of `256:00` and more.
    pub m: u16,
    /// 秒
    pub s: u16,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::{parse_csa_file, parse_ki2_file, parse_kif_file};
    use jsonschema::JSONSchema;
    use std::ffi::OsStr;
    use std::fs::{DirEntry, File};
    use std::io::{BufReader, Result};
    use std::path::Path;

    fn load_schema() -> Result<JSONSchema> {
        let file = File::open("data/specification/json-kifu-format.schema.json")?;
        Ok(JSONSchema::compile(
            &serde_json::from_reader::<_, serde_json::Value>(BufReader::new(file))
                .expect("failed to parse JSON"),
        )
        .expect("failed to compile schema"))
    }

    fn visit_dirs(dir: &Path, cb: &dyn Fn(&DirEntry) -> Result<()>) -> Result<()> {
        if dir.is_dir() {
            for entry in dir.read_dir()? {
                let entry = entry?;
                let path = entry.path();
                if path.is_dir() {
                    visit_dirs(&path, cb)?;
                } else {
                    cb(&entry)?;
                }
            }
        }
        Ok(())
    }

    #[test]
    fn deserialize() -> Result<()> {
        visit_dirs(Path::new("data/tests"), &|entry: &DirEntry| -> Result<()> {
            let path = entry.path();
            if path.extension() == Some(OsStr::new("json")) {
                let file = File::open(&path)?;
                let reader = BufReader::new(file);
                assert!(
                    serde_json::from_reader::<_, JsonKifuFormat>(reader).is_ok(),
                    "failed to deserialize: {}",
                    path.display()
                );
            }
            Ok(())
        })
    }

    #[test]
    fn validate_default() -> Result<()> {
        let schema = load_schema()?;

        let value = serde_json::to_value(JsonKifuFormat::default()).expect("failed to serialize");
        let result = schema.validate(&value);
        if let Err(mut errors) = result {
            if let Some(err) = errors.next() {
                panic!("{:?}", err);
            }
        }
        Ok(())
    }

    #[test]
    fn validate_from_files() -> Result<()> {
        let schema = load_schema()?;

        visit_dirs(Path::new("data/tests"), &|entry: &DirEntry| -> Result<()> {
            let path = entry.path();
            let jkf = match path.extension().and_then(|s| s.to_str()) {
                Some("csa") => parse_csa_file(&path).unwrap_or_else(|err| {
                    panic!("failed to parse csa file {}: {err:?}", path.display())
                }),
                Some("kif") => parse_kif_file(&path).unwrap_or_else(|err| {
                    panic!("failed to parse kif file {}: {err:?}", path.display())
                }),
                Some("ki2") => parse_ki2_file(&path).unwrap_or_else(|err| {
                    panic!("failed to parse ki2 file {}: {err:?}", path.display())
                }),
                _ => return Ok(()),
            };
            let value = serde_json::to_value(&jkf).expect("failed to serialize");
            let result = schema.validate(&value);
            if let Err(mut errors) = result {
                if let Some(err) = errors.next() {
                    panic!("error on {}: {:?}", path.display(), err);
                }
            }
            Ok(())
        })
    }
}
