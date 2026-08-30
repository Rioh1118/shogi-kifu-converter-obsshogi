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

/// An error that can occur while normalizing [`JsonKifuFormat`](crate::jkf::JsonKifuFormat)
#[derive(Error, Debug, PartialEq)]
pub enum NormalizeError {
    /// Couldn't disambiguous the [`jkf::MoveMoveFormat.from`](crate::jkf::MoveMoveFormat::from)
    #[error("Move `from` is ambiguous: {0:?}")]
    AmbiguousMoveFrom(Vec<shogi_core::Square>),
    /// [`shogi_core::PartialPosition::last_move()`] is required if [`jkf::MoveMoveFormat::same`](crate::jkf::MoveMoveFormat::same) is not `None`
    #[error("No previous move")]
    NoLastMove,
    /// There are no pieces at the [`shogi_core::Square`]
    #[error("No pieces at {0:?}")]
    NoPieceAt(shogi_core::Square),
    /// [`shogi_core::PartialPosition::make_move()`] was failed for the [`shogi_core::Move`]
    #[error("Invalid move: {0:?}")]
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

impl From<ConvertError> for NormalizeError {
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
