//! The handicap table.
//!
//! Every handicap is the even game minus a set of the upper hand's pieces, so
//! that set is the whole of the knowledge. The KIF name, the CSA `PI` spelling,
//! the squares to clear and the board to compare against are all views of it.
//!
//! One table, not one per direction. The KIF parser, the KIF writer, the CSA
//! writer and the normalizer all need it, and four copies disagree: a name the
//! parser accepts and a writer cannot spell is a panic in production. Here a
//! new entry is a compile error in each place that has to handle it.
//!
//! Board layout and the pieces removed come from `research/40-handicap.md`
//! (R-HC-003).

use crate::jkf::{Color, Initial, Kind, Piece, Preset};
use crate::normalizer::HIRATE_BOARD;

/// A handicap: the name KIF gives it and what the upper hand takes off.
pub(crate) struct Handicap {
    pub(crate) preset: Preset,
    /// The word that follows `手合割：`.
    pub(crate) kif_name: &'static str,
    /// The pieces removed from the even game, as `(file, rank, kind)`.
    ///
    /// The order is the one CSA's `PI` spelling uses: rook, bishop, lances,
    /// knights, silvers, golds.
    pub(crate) removed: &'static [(u8, u8, Kind)],
}

const HI: (u8, u8, Kind) = (8, 2, Kind::HI);
const KA: (u8, u8, Kind) = (2, 2, Kind::KA);
const KY_RIGHT: (u8, u8, Kind) = (9, 1, Kind::KY);
const KY_LEFT: (u8, u8, Kind) = (1, 1, Kind::KY);
const KE_RIGHT: (u8, u8, Kind) = (8, 1, Kind::KE);
const KE_LEFT: (u8, u8, Kind) = (2, 1, Kind::KE);
const GI_RIGHT: (u8, u8, Kind) = (7, 1, Kind::GI);
const GI_LEFT: (u8, u8, Kind) = (3, 1, Kind::GI);
const KI_RIGHT: (u8, u8, Kind) = (6, 1, Kind::KI);
const KI_LEFT: (u8, u8, Kind) = (4, 1, Kind::KI);

/// Every handicap KIF names, `その他` aside.
///
/// `左`/`右` are from the upper hand's seat, so the upper hand's left is file 1
/// (R-HC-002). That is why 香落ち takes 1一 and 左五枚落ち takes 2一.
pub(crate) const HANDICAPS: [Handicap; 16] = [
    Handicap {
        preset: Preset::PresetHirate,
        kif_name: "平手",
        removed: &[],
    },
    Handicap {
        preset: Preset::PresetKY,
        kif_name: "香落ち",
        removed: &[KY_LEFT],
    },
    Handicap {
        preset: Preset::PresetKYR,
        kif_name: "右香落ち",
        removed: &[KY_RIGHT],
    },
    Handicap {
        preset: Preset::PresetKA,
        kif_name: "角落ち",
        removed: &[KA],
    },
    Handicap {
        preset: Preset::PresetHI,
        kif_name: "飛車落ち",
        removed: &[HI],
    },
    Handicap {
        preset: Preset::PresetHIKY,
        kif_name: "飛香落ち",
        removed: &[HI, KY_LEFT],
    },
    Handicap {
        preset: Preset::Preset2,
        kif_name: "二枚落ち",
        removed: &[HI, KA],
    },
    Handicap {
        preset: Preset::Preset3,
        kif_name: "三枚落ち",
        removed: &[HI, KA, KY_LEFT],
    },
    Handicap {
        preset: Preset::Preset4,
        kif_name: "四枚落ち",
        removed: &[HI, KA, KY_RIGHT, KY_LEFT],
    },
    Handicap {
        preset: Preset::Preset5,
        kif_name: "五枚落ち",
        removed: &[HI, KA, KY_RIGHT, KY_LEFT, KE_RIGHT],
    },
    Handicap {
        preset: Preset::Preset5L,
        kif_name: "左五枚落ち",
        removed: &[HI, KA, KY_RIGHT, KY_LEFT, KE_LEFT],
    },
    Handicap {
        preset: Preset::Preset6,
        kif_name: "六枚落ち",
        removed: &[HI, KA, KY_RIGHT, KY_LEFT, KE_RIGHT, KE_LEFT],
    },
    Handicap {
        preset: Preset::Preset7R,
        kif_name: "右七枚落ち",
        removed: &[HI, KA, KY_RIGHT, KY_LEFT, KE_RIGHT, KE_LEFT, GI_RIGHT],
    },
    Handicap {
        preset: Preset::Preset7L,
        kif_name: "左七枚落ち",
        removed: &[HI, KA, KY_RIGHT, KY_LEFT, KE_RIGHT, KE_LEFT, GI_LEFT],
    },
    Handicap {
        preset: Preset::Preset8,
        kif_name: "八枚落ち",
        removed: &[
            HI, KA, KY_RIGHT, KY_LEFT, KE_RIGHT, KE_LEFT, GI_RIGHT, GI_LEFT,
        ],
    },
    Handicap {
        preset: Preset::Preset10,
        kif_name: "十枚落ち",
        removed: &[
            HI, KA, KY_RIGHT, KY_LEFT, KE_RIGHT, KE_LEFT, GI_RIGHT, GI_LEFT, KI_RIGHT, KI_LEFT,
        ],
    },
];

/// The entry for `preset`, or `None` for [`Preset::PresetOther`], whose board
/// is carried in the data rather than named.
pub(crate) fn lookup(preset: Preset) -> Option<&'static Handicap> {
    HANDICAPS.iter().find(|h| h.preset == preset)
}

/// The KIF names, longest first, so a name is never cut short by a prefix of
/// itself (`右香落ち` before `香落ち`).
pub(crate) fn names_longest_first() -> Vec<&'static Handicap> {
    let mut all: Vec<_> = HANDICAPS.iter().collect();
    all.sort_by_key(|h| std::cmp::Reverse(h.kif_name.len()));
    all
}

/// Whose turn it is at the start.
///
/// The upper hand moves first in every handicap; no piece toss (R-HC-001).
pub(crate) fn side_to_move(preset: Preset) -> Color {
    match preset {
        Preset::PresetHirate => Color::Black,
        _ => Color::White,
    }
}

/// Whose turn it is at ply 1.
///
/// An explicit board states it; otherwise the handicap decides, and only the
/// even game starts with Black (R-HC-001).
pub(crate) fn starting_side(initial: Option<&Initial>) -> Color {
    match initial {
        None => Color::Black,
        Some(initial) => match &initial.data {
            Some(data) => data.color,
            None => side_to_move(initial.preset),
        },
    }
}

/// Whose turn it is at `ply`, given that `start` plays ply 1.
///
/// The parity of the ply does not decide this on its own: the upper hand moves
/// first in every handicap (R-HC-001), so a handicap record has White at every
/// odd ply. Reading the side off the parity alone swaps 先手 and 後手 for the
/// whole record.
pub(crate) fn side_to_move_at_ply(start: Color, ply: usize) -> Color {
    if ply % 2 == 1 {
        start
    } else {
        match start {
            Color::Black => Color::White,
            Color::White => Color::Black,
        }
    }
}

/// The board `preset` starts from, or `None` for [`Preset::PresetOther`].
pub(crate) fn board(preset: Preset) -> Option<[[Piece; 9]; 9]> {
    let handicap = lookup(preset)?;
    let mut board = HIRATE_BOARD;
    for &(file, rank, _) in handicap.removed {
        board[file as usize - 1][rank as usize - 1] = Piece::empty();
    }
    Some(board)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::converter::{ToCsa, ToKi2, ToKif};
    use crate::parser::parse_kif_str;

    /// One row of R-HC-003: the KIF name, the preset, and the squares cleared.
    type SpecRow = (&'static str, Preset, &'static [(u8, u8, Kind)]);

    /// `research/40-handicap.md` R-HC-003, transcribed by hand.
    ///
    /// The value of this is that it does not come from `HANDICAPS`. An
    /// expectation derived from the table under test passes when a piece is
    /// dropped from an entry, and passes when 五枚落ち and 左五枚落ち are
    /// swapped — which is the one mistake R-HC-003 warns about by name, so it
    /// is spelled out: `5` takes 8一桂 (the upper hand's right knight) and
    /// `5_L` takes 2一桂 (its left).
    ///
    /// The order of the squares is deliberately not checked. R-HC-003 says the
    /// order pieces are listed in is unspecified.
    const R_HC_003: [SpecRow; 16] = [
        ("平手", Preset::PresetHirate, &[]),
        ("香落ち", Preset::PresetKY, &[(1, 1, Kind::KY)]),
        ("右香落ち", Preset::PresetKYR, &[(9, 1, Kind::KY)]),
        ("角落ち", Preset::PresetKA, &[(2, 2, Kind::KA)]),
        ("飛車落ち", Preset::PresetHI, &[(8, 2, Kind::HI)]),
        (
            "飛香落ち",
            Preset::PresetHIKY,
            &[(8, 2, Kind::HI), (1, 1, Kind::KY)],
        ),
        (
            "二枚落ち",
            Preset::Preset2,
            &[(8, 2, Kind::HI), (2, 2, Kind::KA)],
        ),
        (
            "三枚落ち",
            Preset::Preset3,
            &[(8, 2, Kind::HI), (2, 2, Kind::KA), (1, 1, Kind::KY)],
        ),
        (
            "四枚落ち",
            Preset::Preset4,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
            ],
        ),
        (
            "五枚落ち",
            Preset::Preset5,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (8, 1, Kind::KE),
            ],
        ),
        (
            "左五枚落ち",
            Preset::Preset5L,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (2, 1, Kind::KE),
            ],
        ),
        (
            "六枚落ち",
            Preset::Preset6,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (8, 1, Kind::KE),
                (2, 1, Kind::KE),
            ],
        ),
        (
            "右七枚落ち",
            Preset::Preset7R,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (8, 1, Kind::KE),
                (2, 1, Kind::KE),
                (7, 1, Kind::GI),
            ],
        ),
        (
            "左七枚落ち",
            Preset::Preset7L,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (8, 1, Kind::KE),
                (2, 1, Kind::KE),
                (3, 1, Kind::GI),
            ],
        ),
        (
            "八枚落ち",
            Preset::Preset8,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (8, 1, Kind::KE),
                (2, 1, Kind::KE),
                (7, 1, Kind::GI),
                (3, 1, Kind::GI),
            ],
        ),
        (
            "十枚落ち",
            Preset::Preset10,
            &[
                (8, 2, Kind::HI),
                (2, 2, Kind::KA),
                (9, 1, Kind::KY),
                (1, 1, Kind::KY),
                (8, 1, Kind::KE),
                (2, 1, Kind::KE),
                (7, 1, Kind::GI),
                (3, 1, Kind::GI),
                (6, 1, Kind::KI),
                (4, 1, Kind::KI),
            ],
        ),
    ];

    #[test]
    fn the_table_matches_the_specification() {
        let mut seen = std::collections::BTreeSet::new();
        for (kif_name, preset, removed) in R_HC_003 {
            let entry = lookup(preset).unwrap_or_else(|| panic!("{kif_name} is missing"));
            assert_eq!(kif_name, entry.kif_name, "the KIF name for {preset:?}");
            let want: std::collections::BTreeSet<_> = removed
                .iter()
                .map(|&(f, r, k)| (f, r, format!("{k:?}")))
                .collect();
            let got: std::collections::BTreeSet<_> = entry
                .removed
                .iter()
                .map(|&(f, r, k)| (f, r, format!("{k:?}")))
                .collect();
            assert_eq!(want.len(), removed.len(), "{kif_name} lists a square twice");
            assert_eq!(want, got, "the pieces {kif_name} takes off");
            seen.insert(kif_name);
        }
        assert_eq!(
            seen.len(),
            HANDICAPS.len(),
            "R-HC-003 lists {} handicaps, the table holds {}",
            seen.len(),
            HANDICAPS.len()
        );
    }

    /// Every name the KIF parser accepts has to survive every write path
    /// (R-HC-005). A name a writer cannot spell is a panic in production.
    #[test]
    fn every_handicap_round_trips() {
        for handicap in HANDICAPS {
            let src = format!(
                "手合割：{}\n手数----指手---------消費時間--\n",
                handicap.kif_name
            );
            let jkf = parse_kif_str(&src).unwrap_or_else(|e| panic!("{}: {e}", handicap.kif_name));
            assert_eq!(
                handicap.preset,
                jkf.initial.expect("an initial position").preset,
                "reading {}",
                handicap.kif_name
            );
            // None of these may panic.
            let kif = jkf.try_to_kif_owned().expect("writes KIF");
            let _ = jkf.try_to_ki2_owned();
            let csa = jkf.try_to_csa_owned().expect("writes CSA");

            assert!(
                kif.contains(handicap.kif_name),
                "{} missing from {kif:?}",
                handicap.kif_name
            );
            let expected_pi: String = std::iter::once("PI".to_owned())
                .chain(
                    handicap
                        .removed
                        .iter()
                        .map(|&(file, rank, kind)| format!("{file}{rank}{kind:?}")),
                )
                .collect();
            assert!(
                csa.lines().any(|l| l == expected_pi),
                "{} expected {expected_pi:?} in {csa:?}",
                handicap.kif_name
            );
            let back = parse_kif_str(&kif).unwrap_or_else(|e| panic!("{}: {e}", handicap.kif_name));
            assert_eq!(
                jkf.initial, back.initial,
                "round trip {}",
                handicap.kif_name
            );
        }
    }

    /// The `PI` block is followed by the side to move, and getting it wrong
    /// makes every handicap start as Black's — the lower hand's first move
    /// becomes the upper hand's, and the whole record replays as the wrong
    /// colour. Writing `+` unconditionally passed every other test.
    #[test]
    fn the_csa_preset_block_states_the_side_to_move() {
        for handicap in HANDICAPS {
            let src = format!(
                "手合割：{}\n手数----指手---------消費時間--\n",
                handicap.kif_name
            );
            let jkf = parse_kif_str(&src).unwrap_or_else(|e| panic!("{}: {e}", handicap.kif_name));
            let csa = jkf.try_to_csa_owned().expect("writes CSA");
            let want = if handicap.preset == Preset::PresetHirate {
                "+"
            } else {
                "-"
            };
            assert!(
                csa.lines().any(|l| l == want),
                "{} expected a {want:?} line in {csa:?}",
                handicap.kif_name
            );
        }
    }

    /// The upper hand moves first in every handicap (R-HC-001).
    #[test]
    fn only_the_even_game_starts_with_black() {
        for handicap in HANDICAPS {
            let want = if handicap.preset == Preset::PresetHirate {
                Color::Black
            } else {
                Color::White
            };
            assert_eq!(want, side_to_move(handicap.preset), "{}", handicap.kif_name);
        }
    }
}
