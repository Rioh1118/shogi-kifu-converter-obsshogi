//! # shogi-kifu-converter
//!
//! A Rust library that defines structs compatible with [json-kifu-format](https://github.com/na2hiro/json-kifu-format), containing parsers and converters for Shogi kifu (game record) for converting to and from json-kifu-format.
//! And, it also provides conversion from `JsonKifuFormat` type to [`shogi_core`](https://crates.io/crates/shogi_core)'s `Position` type.
//!
//! ## About json-kifu-format (JKF)
//!
//! See [https://github.com/na2hiro/json-kifu-format](https://github.com/na2hiro/json-kifu-format).

// The only consumer pins this crate by git tag, so the public surface is the
// contract. An undocumented `pub` is a contract nobody can read.
#![warn(missing_docs)]
// The consumer is a Tauri command, so a panic takes the application down. Every
// input here comes from a file written by someone else, which means none of
// these may sit on a path an input can reach. Tests are exempt.
#![cfg_attr(not(test), deny(clippy::unimplemented, clippy::todo))]
#![cfg_attr(not(test), deny(clippy::unreachable, clippy::panic))]
#![cfg_attr(not(test), deny(clippy::unwrap_used, clippy::expect_used))]
// The lints above only see this crate. A default method on an external trait
// brings its own body — and its `debug_assert!` — to every call site, where
// they cannot reach it. `clippy.toml` names the ones that matter.
#![deny(clippy::disallowed_methods)]

pub mod converter;
mod csa;
pub mod error;
mod handicap;
pub mod jkf;
mod normalizer;
mod notation;
pub mod parser;
mod shogi_core;

/// An alias for [`jkf::JsonKifuFormat`]
pub type JKF = jkf::JsonKifuFormat;
