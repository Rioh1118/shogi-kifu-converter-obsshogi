//! [`JsonKifuFormat`] types
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

/// A representation of an initial state preset
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
///
/// `#[non_exhaustive]`: besides JKF's fourteen, this crate adds the words KIF
/// and CSA V3.0 have and JKF does not (D1, D28, D29), and another such word
/// is a new variant, not a breaking change. Match with a `_` arm.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
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
    /// 先手の不戦勝 — **JKF 標準にはないバリアント**（R-KIF-007 / D1）
    #[serde(rename = "FUSENSHO")]
    SpecialFusensho,
    /// 先手の不戦敗 — **JKF 標準にはないバリアント**（R-KIF-007 / D1）
    #[serde(rename = "FUSENPAI")]
    SpecialFusenpai,
    /// 封じ手 — **JKF 標準にはないバリアント**（GAP-035 / D28）
    ///
    /// The game is adjourned with the next move sealed. Not in R-KIF-007, but
    /// written by the records titles publish, and read by tsshogi and
    /// Kifu-for-JS.
    #[serde(rename = "FUUJITE")]
    SpecialFuujite,
    /// 最大手数（CSA V3.0 の `%MAX_MOVES`）— **JKF 標準にはないバリアント**（GAP-041 / D29）
    #[serde(rename = "MAX_MOVES")]
    SpecialMaxMoves,
}

impl Kind {
    /// Every piece, so that a table over them can be checked for gaps.
    ///
    /// The spellings live in [`crate::notation`]; what this is for is the test
    /// that puts each piece on the board, in a hand and in a move and reads the
    /// record back (`converter::tests::every_piece_survives_every_format`). A
    /// writer's table that no reader is tied to drifts in silence: `杏` and `全`
    /// could be swapped with every test still green.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 14] = [
        Self::FU,
        Self::KY,
        Self::KE,
        Self::GI,
        Self::KI,
        Self::KA,
        Self::HI,
        Self::OU,
        Self::TO,
        Self::NY,
        Self::NK,
        Self::NG,
        Self::UM,
        Self::RY,
    ];
}

impl Relative {
    /// Every way a move can say which piece made it, so that a table over them
    /// can be checked for gaps (R-NOT-004).
    ///
    /// Longest first, because that is the order the reader has to try them in:
    /// `左上` before `左`, or it takes the `左` and leaves `上` for the next rule
    /// (`parser::ki2::relative_word_here` walks this).
    ///
    /// Not `#[cfg(test)]`. A table the production code cannot see is a table the
    /// production code writes out again.
    pub(crate) const ALL: [Self; 13] = [
        Self::LU,
        Self::LM,
        Self::LD,
        Self::RU,
        Self::RM,
        Self::RD,
        Self::L,
        Self::C,
        Self::R,
        Self::U,
        Self::M,
        Self::D,
        Self::H,
    ];

    /// Where `self` sits in [`Self::ALL`].
    ///
    /// Written out rather than `self as usize`, because [`Self::ALL`] is in
    /// longest-spelling order and not declaration order. Exhaustive, so a
    /// variant added to the enum stops the build here, and
    /// `parser::ki2::tests::every_relative_word_reads_back_as_itself` checks the
    /// two against each other.
    #[cfg(test)]
    pub(crate) const fn ordinal(self) -> usize {
        match self {
            Self::LU => 0,
            Self::LM => 1,
            Self::LD => 2,
            Self::RU => 3,
            Self::RM => 4,
            Self::RD => 5,
            Self::L => 6,
            Self::C => 7,
            Self::R => 8,
            Self::U => 9,
            Self::M => 10,
            Self::D => 11,
            Self::H => 12,
        }
    }
}

impl MoveSpecial {
    /// Where `self` sits in [`Self::ALL`].
    ///
    /// The enum is unit-only, so the discriminant is the position in the
    /// declaration and [`Self::ALL`] is checked against it
    /// (`every_special_is_in_the_list_of_every_special`).
    #[cfg(test)]
    pub(crate) const fn ordinal(self) -> usize {
        self as usize
    }

    /// Every special move, so that a table over them can be checked for gaps.
    ///
    /// A `match` on this enum is exhaustive by construction; a hand-written list
    /// of the words is not, and the reader's `KIF_SPECIAL_WORDS` is one
    /// (`parser::kakinoki::tests::the_reader_looks_for_every_word_the_writer_writes`).
    ///
    /// This list is hand-written too, and nothing in the language makes it
    /// follow the enum: a variant appended at the end leaves it short and the
    /// build passes. What holds it is that this vocabulary is not ours to
    /// extend — 14 special moves are JKF's and the other two are what KIF spells
    /// (R-KIF-007, D7) — plus `every_special_is_in_the_list_of_every_special`,
    /// which catches a variant inserted anywhere but the end.
    #[cfg(test)]
    pub(crate) const ALL: [Self; 18] = [
        Self::SpecialToryo,
        Self::SpecialChudan,
        Self::SpecialSennichite,
        Self::SpecialTimeUp,
        Self::SpecialIllegalMove,
        Self::SpecialIllegalActionBlack,
        Self::SpecialIllegalActionWhite,
        Self::SpecialJishogi,
        Self::SpecialKachi,
        Self::SpecialHikiwake,
        Self::SpecialMatta,
        Self::SpecialTsumi,
        Self::SpecialFuzumi,
        Self::SpecialError,
        Self::SpecialFusensho,
        Self::SpecialFusenpai,
        Self::SpecialFuujite,
        Self::SpecialMaxMoves,
    ];
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

impl Piece {
    pub(crate) const fn empty() -> Self {
        Self {
            color: None,
            kind: None,
        }
    }
}

impl Kind {
    pub(crate) fn promoted(self) -> Self {
        match self {
            Kind::FU => Kind::TO,
            Kind::KY => Kind::NY,
            Kind::KE => Kind::NK,
            Kind::GI => Kind::NG,
            Kind::KA => Kind::UM,
            Kind::HI => Kind::RY,
            _ => self,
        }
    }
    pub(crate) fn unpromoted(self) -> Self {
        match self {
            Kind::TO => Kind::FU,
            Kind::NY => Kind::KY,
            Kind::NK => Kind::KE,
            Kind::NG => Kind::GI,
            Kind::UM => Kind::KA,
            Kind::RY => Kind::HI,
            _ => self,
        }
    }
}

impl Hand {
    pub(crate) const fn empty() -> Self {
        Hand {
            FU: 0,
            KY: 0,
            KE: 0,
            GI: 0,
            KI: 0,
            KA: 0,
            HI: 0,
        }
    }
    /// The slot for `kind`, or `None` for a king or a promoted piece.
    ///
    /// A king never goes to the hand (R-CSA-006), and a promoted piece is turned
    /// back over when captured, so neither can sit in one. A broken CSA can
    /// still say they do, and that has to come back as an error rather than
    /// take the process down.
    fn slot(&mut self, kind: Kind) -> Option<&mut u8> {
        Some(match kind {
            Kind::FU => &mut self.FU,
            Kind::KY => &mut self.KY,
            Kind::KE => &mut self.KE,
            Kind::GI => &mut self.GI,
            Kind::KI => &mut self.KI,
            Kind::KA => &mut self.KA,
            Kind::HI => &mut self.HI,
            _ => return None,
        })
    }
    /// Adds one to `kind`. `None` if it cannot be held, or would overflow.
    pub(crate) fn increment(&mut self, kind: Kind) -> Option<()> {
        let slot = self.slot(kind)?;
        *slot = slot.checked_add(1)?;
        Some(())
    }
    /// Takes one from `kind`. `None` if it cannot be held, or none are left —
    /// which means the board holds more of that piece than a set contains.
    pub(crate) fn decrement(&mut self, kind: Kind) -> Option<()> {
        let slot = self.slot(kind)?;
        *slot = slot.checked_sub(1)?;
        Some(())
    }
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
    /// A move does, and so does a special move — KIF numbers `投了` as a line like
    /// any other, and KI2 counts it when it says `まで<N>手で`. A node carrying
    /// only comments or only a time does not: there is nothing in the file for a
    /// reader to number.
    ///
    /// Both writers and the KI2 reader have to agree on this, or a `変化：N手`
    /// block attaches to the wrong move and the special move names the wrong winner.
    pub(crate) fn occupies_ply(&self) -> bool {
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

    // `ALL` is the list every table over special moves is checked against, and it is
    // hand-written. `ordinal` is exhaustive, so a variant added to the enum
    // stops the build there; this pairs the two so the list cannot be short.
    #[test]
    fn every_special_is_in_the_list_of_every_special() {
        for (i, special) in MoveSpecial::ALL.into_iter().enumerate() {
            assert_eq!(i, special.ordinal(), "{special:?} is out of place");
        }
    }
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
