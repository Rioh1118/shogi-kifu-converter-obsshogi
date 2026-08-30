//! Error definitions

use thiserror::Error;

/// An error that can occur while converting from/into [`shogi_core::Position`]
#[derive(Error, Debug, PartialEq)]
pub enum ConvertError {
    /// Board data is required if `preset` is [`PresetOther`](crate::jkf::Preset::PresetOther)
    #[error("Invalid initial board: no data with preset `OTHER`")]
    InitialBoardNoDataWithPresetOTHER,
    /// [`shogi_core::Square::new()`] was failed for the `(file, rank)`
    #[error("Invalid (file, rank) for `Square`: {0:?}")]
    InvalidSquare((u8, u8)),
    /// Invalid [`shogi_core::PieceKind`] for [`shogi_core::Hand`]
    #[error("Invalid piece kind for `Hand`: {0:?}")]
    InvalidHandPiece(shogi_core::PieceKind),
    /// The starting position names a handicap with no board (R-HC-004)
    #[error("No board for preset: {0:?}")]
    UnknownPreset(crate::jkf::Preset),
    /// A number the notation has no characters for
    ///
    /// A file or a rank outside 1-9, or a count in hand above 18. Writing it
    /// anyway would produce a file this crate's own reader cannot read back.
    #[error("The notation cannot spell the number {0}")]
    UnspellableNumber(u8),
    /// More than one piece could have made the move and the notation has no
    /// suffix that tells them apart (R-NOT-004 stage 3)
    #[error("The notation cannot tell this move apart: {}", crate::notation::MoveText(*.0))]
    UnspellableMove(shogi_core::Move),
    /// The sink refused the write
    ///
    /// Writing into a `String` never fails, so this only reaches a caller that
    /// supplied its own sink.
    #[error("Failed to write")]
    Write,
    /// An error that occurred while normalizing [`JsonKifuFormat`](crate::jkf::JsonKifuFormat)
    ///
    /// Boxed rather than flattened to its message: the two enums refer to each
    /// other, and a caller that has to tell an illegal move (valid input under
    /// R-RULE-002) from a board it cannot build needs the variant, not prose.
    #[error("Failed to normalize: {0}")]
    Normalize(Box<NormalizeError>),
}

impl From<NormalizeError> for ConvertError {
    fn from(err: NormalizeError) -> Self {
        Self::Normalize(Box::new(err))
    }
}

impl From<std::fmt::Error> for ConvertError {
    fn from(_: std::fmt::Error) -> Self {
        Self::Write
    }
}

/// An error that can occur while normalizing [`JsonKifuFormat`](crate::jkf::JsonKifuFormat),
/// and the move it happened on.
///
/// The move is half the answer. A caller that is handed a directory of kifu and
/// told "invalid move" cannot act on it; told which ply, it can show the line.
///
/// A branch that fails to normalize is kept as it was rather than reported
/// (R-RULE-002 — a recorded illegal move is valid input, and dropping the
/// branch would edit the file). So a `ply` here always counts along the main
/// line.
#[derive(Debug, PartialEq)]
pub struct NormalizeError {
    /// Which move, indexed the way JKF indexes `moves`: index 0 is the initial
    /// position's slot and holds no move, so the first move is 1 (R-JKF-001).
    ///
    /// 0 means the failure was in the initial position itself, before any move.
    pub ply: usize,
    /// What went wrong.
    pub kind: NormalizeErrorKind,
}

impl NormalizeError {
    /// The error `kind` reports for `ply`.
    pub(crate) fn at(ply: usize, kind: impl Into<NormalizeErrorKind>) -> Self {
        Self {
            ply,
            kind: kind.into(),
        }
    }
}

impl std::fmt::Display for NormalizeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self.ply {
            0 => write!(f, "{} in the initial position", self.kind),
            ply => write!(f, "{} at ply {ply}", self.kind),
        }
    }
}

impl std::error::Error for NormalizeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.kind)
    }
}

/// What went wrong while normalizing. See [`NormalizeError`] for where.
#[derive(Error, Debug, PartialEq)]
pub enum NormalizeErrorKind {
    /// Couldn't disambiguous the [`jkf::MoveMoveFormat.from`](crate::jkf::MoveMoveFormat::from)
    #[error("Move `from` is ambiguous: {}", crate::notation::Coordinates(.0))]
    AmbiguousMoveFrom(Vec<shogi_core::Square>),
    /// [`shogi_core::PartialPosition::last_move()`] is required if [`jkf::MoveMoveFormat::same`](crate::jkf::MoveMoveFormat::same) is not `None`
    #[error("No previous move")]
    NoLastMove,
    /// There are no pieces at the [`shogi_core::Square`]
    #[error("No pieces at {}", crate::notation::Coordinate(*.0))]
    NoPieceAt(shogi_core::Square),
    /// [`shogi_core::PartialPosition::make_move()`] was failed for the [`shogi_core::Move`]
    #[error("Invalid move: {}", crate::notation::MoveText(*.0))]
    MakeMoveFailed(shogi_core::Move),
    /// An error that occurred while converting from/into [`shogi_core::Position`]
    ///
    /// Boxed for the same reason as [`ConvertError::Normalize`].
    #[error("Failed to convert: {0}")]
    Convert(Box<ConvertError>),
    /// Incorrect sequence of move colors
    #[error("Invalid color")]
    InvalidColor,
}

impl From<ConvertError> for NormalizeErrorKind {
    fn from(err: ConvertError) -> Self {
        Self::Convert(Box::new(err))
    }
}

/// An error that can occur while parsing kifu data
#[derive(Error, Debug)]
pub enum ParseError {
    /// From [`std::io::Error`]
    #[error(transparent)]
    Io(#[from] std::io::Error),
    /// From [`csa::CsaError`]
    #[error(transparent)]
    Csa(#[from] csa::CsaError),
    /// From [`serde_json::Error`]
    #[error(transparent)]
    Serde(#[from] serde_json::Error),
    /// An error that occurred while converting from [`csa::GameRecord`]
    #[error("Failed to convert from `csa::GameRecord`: {0}")]
    CsaConvert(&'static str),
    /// An error that occurred while parsing a KIF string
    #[error("KIF Error: {0}")]
    Kif(String),
    /// An error that occurred while parsing a KI2 string
    #[error("KI2 Error: {0}")]
    Ki2(String),
    /// Decoding the string had failed
    #[error("Decode Error")]
    Decode,
    /// The file extension was unexpected
    #[error("File extension Error")]
    FileExtension,
    /// An error that occurred while normalizing the parsed record
    ///
    /// Kept as the error rather than its message: a caller has to be able to
    /// tell "this record holds an illegal move" (valid input under R-RULE-002)
    /// from "this file is not a kifu at all", and the consumer picks a text
    /// encoding on that answer.
    #[error("failed to normalize: {0}")]
    Normalize(#[from] NormalizeError),
}
