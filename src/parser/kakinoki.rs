use crate::jkf::*;
use crate::notation::is_padding;
use nom::branch::alt;
use nom::bytes::complete::{tag, take_while, take_while1};
use nom::character::complete::{char, digit1, line_ending, not_line_ending, one_of, satisfy};
use nom::combinator::{eof, map, map_res, opt, peek, value};
use nom::error::{ErrorKind, ParseError, VerboseError};
use nom::multi::{count, many0, many1};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated, tuple};
use nom::IResult;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Information<'a> {
    Preset(Preset),
    HandBlack(Hand, &'a str),
    HandWhite(Hand, &'a str),
    KeyValue(String, String),
    /// A comment or a bookmark standing among the header lines
    /// (R-KIF-010 / R-KIF-011).
    Comment(String),
}

#[derive(Debug, Default, PartialEq, Eq)]
struct InformationData<'a> {
    preset: Option<Preset>,
    /// Each side's hand and the line that stated it, so that the answer to
    /// "does this need a board" and the answer to "which line said so" come from
    /// one place. Two answers is how a refusal came to point at
    /// `先手の持駒：なし` while the line that made it necessary was below.
    hands: [Option<(Hand, &'a str)>; 2],
    map: HashMap<String, String>,
    comments: Vec<String>,
}

impl<'a> InformationData<'a> {
    /// Files what a `…の持駒：` line said.
    ///
    /// A record states each hand once. Stated twice, the two answers this reader
    /// could give are "keep the last" (the first hand is gone without a word)
    /// and "add them up" (pieces appear that nobody wrote) — both silent, which
    /// D1 puts below being refused. An empty hand is the exception: it says
    /// nothing the other line does not, so nothing is lost by taking the other.
    fn state_a_hand(
        &mut self,
        color: Color,
        hand: Hand,
        line: &'a str,
    ) -> Result<(), nom::Err<VerboseError<&'a str>>> {
        let side = &mut self.hands[usize::from(color == Color::White)];
        match side {
            Some((stated, _)) if *stated != Hand::default() && hand != Hand::default() => {
                Err(broken_line(line, "this record states one hand twice"))
            }
            Some((stated, _)) if *stated != Hand::default() => Ok(()),
            _ => {
                *side = Some((hand, line));
                Ok(())
            }
        }
    }

    /// The line of a hand that has nowhere to sit if no board came.
    ///
    /// An empty hand states nothing a preset does not already say, so it is not
    /// a reason to refuse a record and not a line to name.
    fn hand_line_without_board(&self) -> Option<&'a str> {
        self.hands
            .iter()
            .flatten()
            .filter(|(hand, _)| *hand != Hand::default())
            // The first such line in the file, which is the longest tail of it:
            // whichever side it belongs to, the reader is pointed at the first
            // thing it cannot keep.
            .map(|(_, line)| *line)
            .max_by_key(|line| line.len())
    }

    /// What each side holds, for the position.
    fn hands(&self) -> [Hand; 2] {
        self.hands
            .map(|side| side.map_or_else(Hand::default, |(hand, _)| hand))
    }

    /// Takes in what the lines below the ones already read said.
    ///
    /// The hands go through [`Self::state_a_hand`] rather than being merged, so
    /// that a hand stated on both sides of the board is the same "stated twice"
    /// as one stated twice in a row. Merging them here would be a second answer
    /// to the same question, and the two disagreed: one kept the last line and
    /// the other added the two up.
    fn absorb(&mut self, other: Self) -> Result<(), nom::Err<VerboseError<&'a str>>> {
        self.preset = self.preset.or(other.preset);
        for (side, stated) in other.hands.into_iter().enumerate() {
            if let Some((hand, line)) = stated {
                let color = if side == 0 {
                    Color::Black
                } else {
                    Color::White
                };
                self.state_a_hand(color, hand, line)?;
            }
        }
        self.map.extend(other.map);
        self.comments.extend(other.comments);
        Ok(())
    }
}

/// Ends a line, at the end of the file as well as at a newline.
///
/// A text file need not end with a newline, and kifu written by hand and by
/// other software both turn up without one. Requiring `line_ending` drops the
/// last line — a move, a comment or the `まで<N>手で…` — and says nothing about
/// it. Every line parser here is anchored on something it must consume first,
/// so accepting the empty match at the end cannot loop.
fn end_of_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    alt((line_ending, eof))(input)
}

/// The marks a move opens with (R-NOT-001).
///
/// One table, and shared because both formats' readers ask it. The KI2 reader
/// spells its moves with them; `not_move_line` below declines to skip a line
/// that *begins* with one, for KIF as well.
///
/// Only where it begins — past the indentation, so `　▲有利でした` counts as
/// beginning with one just as `▲有利でした` does. Both are refused in KIF as
/// well as in KI2: a mark at the head of a line is how a move opens, and
/// refusing such a line even where it turns out to be prose is the trade D1
/// takes (`parser::tests::a_record_the_reader_stops_in_the_middle_of_is_an_error`).
///
/// A line that merely holds one somewhere — `※▲２六歩が本筋` — is prose to KIF
/// and is skipped, while `ki2::is_prose_only_line` asks `contains` and
/// refuses the whole file. That asymmetry is `research/90-gaps.md` GAP-029,
/// which is waiting on a decision; do not read this table as a promise that KIF
/// keeps such a line.
///
/// `▼`/`▽` and `☗`/`☖` are the same two marks as other programs print them —
/// filled for Black, outlined for White — and tsshogi reads all six. Left out,
/// a `▽３四歩` in the middle of a KI2 line was not refused but dropped, and the
/// record came back one move short (GAP-024, GAP-050). The writers print `▲`/`△`
/// only.
pub(super) const SIDE_MARKS: [(char, Color); 6] = [
    ('▲', Color::Black),
    ('△', Color::White),
    ('▼', Color::Black),
    ('▽', Color::White),
    ('☗', Color::Black),
    ('☖', Color::White),
];

/// The mark a KIF move line may carry in front of the move (R-KIF-005).
///
/// Read and dropped. Whose move it is comes from the run: the first line of one
/// takes its side from the ply (`handicap::side_to_move_at_ply`, which is the
/// parity, corrected for a handicap by R-HC-001), and every line after it from
/// the move before (`kif::move_line`'s `known_side`). A special move line takes a
/// ply without taking a turn, so past one the parity is wrong and only the chain
/// is right — which matters because 反則勝ち names its loser through whose turn
/// it is (R-KIF-007).
///
/// What the mark needs from this reader is to be allowed. tsshogi's move pattern
/// has `[▲△▼▽]?` in it, so a record that opens on the consumer's TS side has to
/// open here too, and refusing it refuses the file whole (D1).
pub(super) fn side_mark(input: &str) -> IResult<&str, char, VerboseError<&str>> {
    satisfy(|c| SIDE_MARKS.iter().any(|(mark, _)| *mark == c))(input)
}

/// The colon a `<keyword>：<value>` line is split on (R-KIF-004).
///
/// Full-width in every published example, and half-width in files all the same:
/// tsshogi matches `[：:]` on the handicap, the hands and the `変化：` header,
/// so a record that opens on the consumer's TS side has to open here too
/// (R-KIF-014, D5). A `手合割:香落ち` this reader does not recognise is not even
/// filed under `header` — it falls to the prose skip, and the handicap is gone
/// without a key to show for it, taking every move's side with it
/// (R-HC-001 / R-RULE-006).
///
/// **Only where a keyword names the line.** tsshogi takes a half-width colon on
/// any metadata line at all (`/^[^ ：:]+[：:]/`), and this reader deliberately
/// does not: a key can be anything (R-KIF-004), so a half-width colon there
/// makes a header line out of every `key: value` in any text —
/// `{"header":{},"moves":[{}]}` becomes a kifu with one header, and the error D1
/// exists to raise reaches nobody (D8, `parse_kif_str`). The cost is that a
/// `棋戦:竜王戦` this reader cannot name is dropped rather than kept as text
/// (`research/90-gaps.md` GAP-031).
pub(super) fn colon(input: &str) -> IResult<&str, char, VerboseError<&str>> {
    one_of(COLONS)(input)
}

/// The one the format itself is written with. The other is tolerance.
const COLON: char = '：';

/// Every character that may separate a keyword from its value.
///
/// One set, so that [`colon`] and the key rule in [`information_line_keyvalue`]
/// cannot answer differently: the key is taken as everything up to one of these,
/// and a character `colon` knows that the key rule does not would be swallowed
/// into the key — leaving nothing for `colon` to find and dropping the line.
const COLONS: &str = "：:";

/// The indentation and column padding a line is written with.
///
/// `nom`'s `space0` in the shape this crate needs it, but over [`is_padding`]:
/// KIF pads its move lines into columns (`   1 ７六歩(77)   ( 0:01/00:00:01)`,
/// R-KIF-008), and a file that reached the user through a web page or a word
/// processor has those columns padded with whatever that tool uses.
///
/// **Every line parser starts with this**, the ones that read a line as much as
/// the one that skips it. Widening only the skip is worse than widening
/// neither: a line the readers can no longer take is then taken by the skip,
/// and the record comes back `Ok` without it.
pub(super) fn padding(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    take_while(is_padding)(input)
}

/// The shapes of the format being read.
///
/// The header block, the board and the side-to-move line are the same in KIF
/// and KI2, but what a line of one of them can have swallowed is not: KIF puts
/// its moves on numbered lines, KI2 on `▲`/`△` runs. A reader that looks for
/// both finds the other format's shape in this one's prose — `※▲２六歩が本筋`
/// after a KIF move, which is a note and not a line — and refuses records with
/// nothing wrong with them.
///
/// So each reader says what its own lines look like, and this module holds what
/// they share: the header block, the board, the shapes above, and the line
/// parsers both formats define the same way.
///
/// Two of those parsers have one reader each today — `blank_line` (KIF) and
/// `program_comment_line` (KI2) — because the other format spells the same rule
/// somewhere else: KIF reads `#` by hand inside `skip_interruptions`, and KI2
/// skips its blank lines inside `move_run`. They are here because the rule is
/// shared (R-KIF-002), not because one format owns it. A rule only one format
/// has does not belong here: reading for both is what refuses records with
/// nothing wrong with them.
#[derive(Clone, Copy)]
pub(super) struct LineShapes {
    /// Whether a header value carries a line of this format. A header value is
    /// free text (R-KIF-004), so this is about what the text carries rather than
    /// where it stops — see [`information_line_keyvalue`].
    pub(super) carries_a_line: fn(&str) -> bool,
    /// Whether text that starts where a line starts opens one.
    pub(super) opens_line: fn(&str) -> bool,
}

/// Where a word on a line ends: padding, the line ending, or the mark a note
/// opens with.
///
/// The special move word on a `まで…` line and the number on a `変化：` line both stop
/// here. One function, because the two would drift: the same character added to
/// one and not the other changes what a special move word is in one place and
/// whether an empty fork block is reported in the other.
pub(super) fn ends_word(c: char) -> bool {
    is_padding(c) || crate::notation::LINE_ENDS.contains(&c) || NOTE_MARKERS.contains(&c)
}

/// What a note opens with.
///
/// Prose about a move puts one of these in front of it — `※▲２六歩が本筋`,
/// `（まで先手良し）`, `【変化】` — which is exactly where
/// [`begins_next_line`] would otherwise look for the newline that was
/// lost. Whatever replaced a newline, it is not one of these.
const NOTE_MARKERS: [char; 8] = ['※', '（', '(', '【', '[', '「', '〈', '＜'];

/// The word a fork declaration opens with (D3).
///
/// One spelling, because the guard in [`opens_fork_header`] and the parser
/// in [`fork_header_ply`] both look for it: a guard that knows a narrower word
/// than the parser silently stops the parser from ever being asked, which is the
/// "counted set ≠ consumed set" failure this pair keeps having.
const FORK_KEYWORD: &str = "変化";

/// The shapes both formats share: a comment, a bookmark, a `#` note, a `変化：`
/// header, a `まで…` special move.
pub(super) fn opens_shared_line(head: &str) -> bool {
    // Past the indentation, for every shape and not just the one whose parser
    // happens to look past it. A predicate that answers `true` for `　変化：2手`
    // and `false` for `　*コメント` has two contracts, and the callers cannot see
    // which one they are getting.
    let head = head.trim_start_matches(is_padding);
    head.starts_with(['*', '&', '#']) || opens_fork_header(head) || head.starts_with("まで")
}

/// Whether `head` is the beginning of a `変化：<N>手` header.
///
/// The number is what makes it one. `変化：` on its own is two characters a
/// sentence can open with.
///
/// Built on [`fork_header_ply`], so that what a reader *counts* as a fork
/// header and what it can *consume* are the same set. Held apart they drift, and
/// the drift shows up in opposite directions: a count wider than the read makes
/// a fork that reads as the main line carrying on (R-JKF-004), and a read
/// wider than the count refuses records the format has always allowed (D17).
///
/// The `starts_with` in front is a guard, not a second answer: it is the
/// parser's own prefix ([`FORK_KEYWORD`]), and it is here because this runs on
/// nearly every line while `fork_header_ply` builds a `VerboseError` on each
/// miss — the cost `kif::skip_interruptions` is hand-written to avoid.
pub(super) fn opens_fork_header(head: &str) -> bool {
    head.trim_start_matches(is_padding)
        .starts_with(FORK_KEYWORD)
        && fork_header_ply(head).is_ok()
}

/// Whether an empty block under this `変化：` line is a fork that went missing.
///
/// True when the number is the last thing the line says — padding, a note
/// marker or the line ending after it. `変化：2手 別案` and `変化：2手（本命）`
/// are true as well: what follows is a note *on* the header, not a sentence the
/// header is part of.
///
/// Not what makes it a header. A number followed by a word is still a fork
/// declaration (D20) — `変化：3手目` names ply 3 — and reading it as prose lets
/// the fork run on as the main line, which changes whose game it is
/// (R-JKF-004).
///
/// What this decides is whether an **empty** block under such a line is worth
/// reporting: `変化：2手` with nothing beneath it is a fork that went missing
/// and D1 says so, while `変化：2手を参照` with nothing beneath it is a note
/// about a fork and there was never anything to lose.
///
/// The padding, the line ending and [`NOTE_MARKERS`] are the same set D17 uses
/// for the end of a line, but **D17 is not the reason** — that rule is about
/// what is left after a line the reader finished with, and this is a question
/// about the head of one. D20 is the decision.
pub(super) fn is_reportable_empty_block(head: &str) -> bool {
    matches!(fork_header_ply(head), Ok((rest, _)) if rest.starts_with(ends_word) || rest.is_empty())
}

/// Reads `変化：<N>手` and returns `N`.
///
/// The same shape [`opens_fork_header`] recognises, so that a spelling one
/// of them takes is a spelling the other takes: a header counted but not
/// consumable is a line that belongs to nobody — not skipped as prose, not read
/// as a header — and the leftover-input check then refuses a record over a line
/// the format has always allowed (D1, D17).
///
/// Full-width digits among them. KIF never uses the number (the tree comes from
/// the ply numbers on the move lines, D3), and KI2 needs it, so folding the
/// spelling here is what lets both read `変化：２手`.
///
/// The number saturates rather than failing (D19). A ply too large for a `usize`
/// is not a spelling this reader cannot read — KIF does not look at the number at
/// all, and KI2 hands it to `attach_fork`, which asks whether a node is there
/// and finds none. Refusing a record over a digit nobody reads is exactly the
/// difference this function exists to close. (Real records need three digits;
/// the ceiling is here so that a damaged one is not fatal, not to support it.)
///
/// Starts past the indentation, so that a `変化：` one column in is the same
/// line as one at column 0 ([`padding`]). The callers all ask the question that
/// way, so consuming it here keeps the counted set and the consumed set equal.
///
/// Ply 0 is read here and refused by the reader that uses it. Plies count from 1
/// (R-JKF-001), so `変化：０手` names no move for a fork to be an alternative
/// to — but only KI2 looks at the number (D3, D19), and refusing the line here
/// would reject KIF records over a digit KIF never reads. See
/// `ki2::fork_header`.
pub(super) fn fork_header_ply(input: &str) -> IResult<&str, usize, VerboseError<&str>> {
    let unreadable = || nom::Err::Error(VerboseError::from_error_kind(input, ErrorKind::Digit));
    let (rest, _) = preceded(pair(padding, tag(FORK_KEYWORD)), colon)(input)?;
    let rest = rest.trim_start_matches(is_padding);
    // Folded a digit at a time, so that the test for "is this a digit" and the
    // arithmetic that assumes it are the same expression. Split apart, widening
    // the first silently underflows the second.
    let mut ply: usize = 0;
    let mut taken = 0;
    for (i, c) in rest.char_indices() {
        let digit = match c {
            '0'..='9' => u32::from(c) - u32::from('0'),
            '０'..='９' => u32::from(c) - u32::from('０'),
            _ => break,
        };
        ply = ply.saturating_mul(10).saturating_add(digit as usize);
        taken = i + c.len_utf8();
    }
    if taken == 0 {
        return Err(unreadable());
    }
    // `手` is what the writers put after the number, but nothing requires it of
    // a reader — tsshogi's `forkRegExp` reads the number and stops.
    let (rest, _) = opt(tag("手"))(&rest[taken..])?;
    Ok((rest, ply))
}

/// Whether `head` is a `変化：<N>手` header and nothing else, to the end of its
/// line.
///
/// Asked where a line has already ended, not where one begins. `変化：2手` in
/// the annotation column of a move line — `  83 投了    変化：2手  ( 0:00)` —
/// is prose about a fork; reading it as the header of one says the record
/// runs into the line below it, which it does not, and refuses the whole file
/// over a note (D17).
///
/// Not the same question as [`is_reportable_empty_block`], which
/// allows a note after the number: `変化：2手（本命）` fills no line but does
/// declare a block. Check which one you mean before calling either.
fn is_whole_line_fork_header(head: &str) -> bool {
    match fork_header_ply(head) {
        Ok((rest, _)) => end_of_line(rest.trim_start_matches(is_padding)).is_ok(),
        Err(_) => false,
    }
}

/// The KIF special move words, longest first so that a word is never cut short by a
/// prefix of itself. [`MoveSpecial::from_kif_word`] holds the mapping.
///
/// Here rather than with the KIF reader because both readers ask: KI2 has to
/// know that `   5 投了` is a line something wrote as a move, so that skipping
/// it as prose does not take the special move with it (D1, `opens_numbered_line`).
const KIF_SPECIAL_WORDS: [&str; 13] = [
    "切れ負け",
    "入玉勝ち",
    "反則負け",
    "反則勝ち",
    "不戦勝",
    "不戦敗",
    "封じ手",
    "千日手",
    "持将棋",
    "投了",
    "中断",
    "詰み",
    "不詰",
];

/// Reads a special move word, returning the word itself.
///
/// The word, not a [`MoveSpecial`], because 反則勝ち does not name its loser:
/// the line says only that the move before it was the foul (R-KIF-007), and
/// whose move that was is the caller's to know. A reader that had to supply a
/// colour here would be choosing one it does not have.
fn special_word(input: &str) -> IResult<&str, &'static str, VerboseError<&str>> {
    for word in KIF_SPECIAL_WORDS {
        if let Ok((rest, _)) = tag::<_, _, VerboseError<&str>>(word)(input) {
            return Ok((rest, word));
        }
    }
    Err(nom::Err::Error(VerboseError::from_error_kind(
        input,
        nom::error::ErrorKind::Alt,
    )))
}

fn move_time_format(input: &str) -> IResult<&str, TimeFormat, VerboseError<&str>> {
    alt((
        map(
            tuple((
                terminated(map_res(digit1, str::parse), tag(":")),
                terminated(map_res(digit1, str::parse), tag(":")),
                map_res(digit1, str::parse),
            )),
            |(h, m, s)| TimeFormat { h: Some(h), m, s },
        ),
        map(
            tuple((
                terminated(map_res(digit1, str::parse), tag(":")),
                map_res(digit1, str::parse),
            )),
            |(m, s)| TimeFormat { h: None, m, s },
        ),
    ))(input)
}

/// The `( 0:03/00:00:03)` a KIF move line may carry after the move or the
/// special move word (R-KIF-007 / R-KIF-008).
///
/// Here rather than beside the reader that consumes it, because the shared
/// question "does this line hold a move" has to be answered with the same shape
/// the reader will take: a `1投了 ( 0:03/00:00:03)` counted as prose is skipped,
/// and the special move that ends the game goes with it (D1).
pub(super) fn move_time(input: &str) -> IResult<&str, Time, VerboseError<&str>> {
    delimited(
        tag("("),
        map(
            separated_pair(
                delimited(padding, move_time_format, padding),
                tag("/"),
                delimited(padding, move_time_format, padding),
            ),
            |(now, total)| Time { now, total },
        ),
        tag(")"),
    )(input)
}

/// Where a move came from: `打` for a drop, or `(77)`.
///
/// R-KIF-005: an origin is `(11)` through `(99)`. `(00)` in particular is CSA's
/// spelling for a drop and this crate's marker for an origin the notation does
/// not state — reading it as either would turn "a square we could not read" into
/// a different move, so it is a `Failure`: the line *is* a move line, and it is
/// broken (D1).
fn move_origin(input: &str) -> IResult<&str, Option<PlaceFormat>, VerboseError<&str>> {
    // A drop has no origin, and JKF says so by leaving `from` out (R-JKF-003).
    // KIF marks it with 打 on every drop (R-KIF-006).
    if let Ok((rest, _)) = tag::<_, _, VerboseError<&str>>("打")(input) {
        return Ok((rest, None));
    }
    // The `(` is the commitment. Past it the line has said it is naming a
    // square, so everything that follows is this move line being broken and not
    // a note that happens to hold a bracket — a `Failure` here, where an `Error`
    // would tell the skip "not my line" and the move would go with it (D21).
    let (rest, _) = tag("(")(input)?;
    let broken = || broken_line(input, "this move's origin square cannot be read");
    let (rest, digits) = digit1::<_, VerboseError<&str>>(rest).map_err(|_| broken())?;
    let (rest, _) = tag::<_, _, VerboseError<&str>>(")")(rest).map_err(|_| broken())?;
    let square: u16 = digits.parse().map_err(|_| broken())?;
    let (x, y) = ((square / 10) as u8, (square % 10) as u8);
    if digits.len() != 2 || !(1..=9).contains(&x) || !(1..=9).contains(&y) {
        return Err(broken());
    }
    Ok((rest, Some(PlaceFormat { x, y })))
}

/// What a `<手数> <指し手>` line says, without the parts that are not on it.
///
/// The colour is not among them. 反則勝ち says the move *before* it was the
/// foul, so the line names its loser only through whose turn it is (R-KIF-007),
/// and that is the caller's to know: `kif::move_line` carries it forward move by
/// move (`known_side`) and falls back to the ply only at the head of a run
/// (`side_mark`).
pub(super) enum NumberedBody {
    /// One of the special move words.
    Special(&'static str),
    /// A move.
    Move {
        /// `None` for 同 — the square is the one the move before went to.
        to: Option<PlaceFormat>,
        /// The piece that moved.
        piece: Kind,
        /// What the record said about promotion, `None` where it said nothing.
        promote: Option<bool>,
        /// `None` for a drop (R-JKF-003).
        from: Option<PlaceFormat>,
    },
}

/// Reads a `<手数> <指し手> [<消費時間>]` line up to the point where the line-end
/// rule (`ends_here`) takes over.
///
/// **The one answer to "what does this line say".** [`opens_numbered_line`] is
/// this function's verdict and holds no spelling of its own, and
/// `kif::move_line` builds its [`MoveFormat`] out of what this returns rather
/// than reading the text a second time. A predicate written separately from the
/// reader drifts from it, and the drift is silent in one direction: a line the
/// reader would have taken but the predicate does not count is skipped as prose,
/// and the move — or the special move that ends the game — is gone from a record that still
/// comes back `Ok` (D1).
///
/// The three answers are the contract:
///
/// - `Ok` — read.
/// - `Err(Failure)` — this *is* one of these lines and it is broken. The caller
///   reports it, and the predicate counts it so that no skip takes it.
/// - `Err(Error)` — not one of these lines at all. Prose.
///
/// It must not call `ends_here`: that asks `LineShapes::opens_line`, which
/// asks [`opens_numbered_line`], which asks this.
pub(super) fn numbered_line(
    input: &str,
) -> IResult<&str, (usize, NumberedBody, Option<Time>), VerboseError<&str>> {
    let line = input;
    let (rest, digits) = preceded(padding, digit1)(input)?;
    let (rest, gap) = padding(rest)?;
    // Whether this line has said it is one of these lines, decided here and
    // nowhere else. **Only what is on the line decides it** — asking whether the
    // file ends here makes the same characters a move line or prose depending on
    // a trailing newline, which is not part of the record (`end_of_line`).
    //
    // A number and then padding is the shape whatever follows, because a word
    // this reader has no meaning for (`   2 パス`) has to reach the
    // leftover-input check rather than be skipped as prose (D1, D8). The cost is
    // a note written in the same shape — `　1 序盤の課題` — refused with it; the
    // two cannot be told apart by what they say (GAP-020). Without padding the
    // move itself has to start, which is what tells `2７六歩` (a move line whose
    // origin went) from `1図以下、先手優勢` (prose).
    // `rest.is_empty()` is the third: a line with no end is not a line this
    // reader can call prose. `   3` as the last thing in a file is a record cut
    // inside a move line, and calling it a note hands the file back as a shorter
    // game (D1). A line that ends — even at the end of the file, with a newline
    // — is judged by what it says.
    let committed = !gap.is_empty() || opens_move_body(rest) || rest.is_empty();
    // A number too long to hold is still a ply number (D19 keeps a record over a
    // number it cannot use), so it is only prose if nothing else on the line
    // said this was a move line.
    let ply: usize = match digits.parse() {
        Ok(ply) => ply,
        Err(_) if committed => {
            return Err(broken_line(
                line,
                "this line's ply number is too large to read",
            ))
        }
        Err(_) => {
            return Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                ErrorKind::Digit,
            )))
        }
    };
    let (rest, body) = alt((map(special_word, NumberedBody::Special), move_body))(rest).map_err(
        |err| match err {
            nom::Err::Error(_) if committed => broken_line(line, "this move cannot be read"),
            err => err,
        },
    )?;
    let (rest, _) = padding(rest)?;
    // No diagnostic of its own: `move_time` is built from `tag` and `map_res`,
    // which raise recoverable errors only, so `opt` takes them all. A clock that
    // cannot be read is left for the line-end rule (D17), which is what carries
    // `( 0:01)` and the other spellings this reader has no shape for.
    let (rest, time) = opt(move_time)(rest)?;
    Ok((rest, (ply, body, time)))
}

fn move_body(input: &str) -> IResult<&str, NumberedBody, VerboseError<&str>> {
    // R-KIF-005 allows the mark in front of the move. Read and dropped — see
    // [`side_mark`] for where the side comes from instead.
    let (input, _) = opt(side_mark)(input)?;
    let (input, to) = move_to(input)?;
    let (input, piece) = piece_kind(input)?;
    // A writer that pads `同` to two columns puts the padding after the piece
    // (`同銀　(79)`, tsshogi's tests) rather than after `同` (`同　銀(79)`).
    // tsshogi's move pattern takes padding there (GAP-038).
    let (input, _) = take_while(is_padding)(input)?;
    // R-KIF-006 asks a *writer* not to spell 不成. Other software spells it, and
    // a reader that counts only what a correct writer produces hands the line to
    // the skip, which drops the move (R-REQ-004).
    let (input, promote) = opt(alt((value(false, tag("不成")), value(true, tag("成")))))(input)?;
    // Past the destination and the piece the line has said what it is, so a
    // missing or unreadable origin is this move line being broken and not a note
    // that happens to quote a move. Both are the same characters — `1同歩` is a
    // note and `2７六歩` is a move line whose origin went — so one of the two has
    // to give, and D1 gives the note (GAP-020 records the trade).
    let (input, from) = move_origin(input).map_err(|err| match err {
        // `move_origin` names what it could not read; only "there is no origin
        // here at all" is left for this to say.
        nom::Err::Error(_) => broken_line(input, "this move has no origin square"),
        err => err,
    })?;
    Ok((
        input,
        NumberedBody::Move {
            to,
            piece,
            promote,
            from,
        },
    ))
}

/// Whether a move starts here — the question that tells a move line a writer
/// left unaligned from prose that opens with a number.
///
/// The shapes the reader itself uses, not a spelling of their own: a mark
/// (R-KIF-005), a destination, or the file of one (`2８` is a move line the file
/// stopped inside of).
fn opens_move_body(rest: &str) -> bool {
    side_mark(rest).is_ok() || move_to(rest).is_ok() || place_x(rest).is_ok()
}

/// Whether `head` is the beginning of a `<手数> <指し手>` line
/// (R-KIF-005 / R-KIF-008).
///
/// **[`numbered_line`]'s verdict, and nothing else.** The two questions — "is
/// this one of my lines" and "what does it say" — have one answer, because a
/// predicate with a spelling of its own drifts from the reader and the drift is
/// silent in one direction: a line the reader would have taken but the predicate
/// does not count is skipped as prose, and the move is gone from a record that
/// still comes back `Ok` (D1, D21).
///
/// Anything except `Err(Error)` counts, so a line that is broken past the point
/// where it said what it is stays here and is reported.
///
/// Shared, though only KIF writes such a line: a KI2 that holds one is a record
/// something went wrong with, and skipping it as prose would take the move with
/// it (D1).
pub(super) fn opens_numbered_line(head: &str) -> bool {
    !matches!(numbered_line(head), Err(nom::Err::Error(_)))
}

/// Whether `tail` — what is left of a line the reader has finished with —
/// begins the line that should have been underneath it.
///
/// `shapes.opens_line` asks the same question of text that starts where a line
/// starts; this one asks it of text that starts where a line ends.
///
/// What follows a finished line is one of two things. Either the line below,
/// whose newline was lost, or an annotation the formats do not define: Kifu for
/// Windows marks moves with a trailing `+`
/// (`data/tests/kif/everyday_20211107.kif`), and a consumed-time spelling this
/// reader has no shape for is left over the same way. Only the first means the
/// record is missing something.
///
/// What was lost is the newline itself, so whatever sits in its place belongs to
/// neither line, and the shapes are looked for once more past a single
/// character — unless that character is one a note opens with
/// ([`NOTE_MARKERS`]). `※ 3 分の1` and `（まで先手良し）` are prose, and looking
/// past the marker reads them as the line below and refuses a whole record over
/// a note.
///
/// Nor is that character ever a newline. A newline still there is one that was
/// never lost, and stepping over it reads the line below as this line's
/// overflow: a record whose only fault is a full-width space before the newline
/// — which `nom`'s `space0` does not take and [`is_padding`] does — is refused
/// whole, with an error naming a line it does not run into.
fn begins_next_line(shapes: LineShapes, tail: &str) -> bool {
    // A `変化：` here has to be the whole of the line it would begin. Everywhere
    // else the question is asked at the head of a line, where a header followed
    // by a note is still a header; here it is asked past the end of one, where
    // the same characters are the note.
    let opens = |head: &str| {
        if opens_fork_header(head) {
            return is_whole_line_fork_header(head);
        }
        (shapes.opens_line)(head)
    };
    let head = tail.trim_start_matches(is_padding);
    if opens(head) {
        return true;
    }
    match head.chars().next() {
        Some(c) if !NOTE_MARKERS.contains(&c) && !crate::notation::LINE_ENDS.contains(&c) => {
            opens(head[c.len_utf8()..].trim_start_matches(is_padding))
        }
        _ => false,
    }
}

/// Requires the line to end where the reader finished reading it, or to trail
/// off into something that is not a line of its own.
///
/// `line` is the whole line, for the error to point at; `rest` is what is left
/// of it.
///
/// A reader that has recognised what a line says owns the line to its end. What
/// follows a line the reader has finished with is not more of that line: the
/// newline that should have separated them is gone, and the line underneath is
/// read as part of this one and lost. One byte does it — a `\n` that arrives as
/// a space joins two move lines, and the second move disappears from a record
/// that still comes back `Ok`, which no caller can tell from a shorter game.
///
/// `Failure` rather than `Error` because every caller sits under an `opt` or an
/// `alt` that swallows a recoverable error and skips the line whole — which is
/// the silence this exists to break.
pub(super) fn ends_here<'a>(
    shapes: LineShapes,
    line: &'a str,
    rest: &'a str,
) -> IResult<&'a str, &'a str, VerboseError<&'a str>> {
    // [`is_padding`] rather than `space0`, which takes neither a full-width
    // space nor anything else outside ASCII: padding it does not recognise is
    // padding the rest of this function then has to explain as something else.
    if let Ok(ended) = end_of_line(rest.trim_start_matches(is_padding)) {
        return Ok(ended);
    }
    if begins_next_line(shapes, rest) {
        return Err(broken_line(line, "this line runs into the one below it"));
    }
    preceded(not_line_ending, end_of_line)(rest)
}

/// The error for a record that cannot be read, pointing at `at` and saying
/// `what`.
///
/// A [`nom::error::VerboseErrorKind::Context`] rather than an `ErrorKind`, whose
/// name (`CrLf`, `Many1`) says what combinator gave up rather than what is wrong
/// with the file. The caller here is someone whose kifu did not open.
///
/// `Failure`: these are all raised from under an `opt` or an `alt` that swallows
/// a recoverable error and carries on without the line.
pub(super) fn broken_line<'a>(at: &'a str, what: &'static str) -> nom::Err<VerboseError<&'a str>> {
    nom::Err::Failure(VerboseError {
        errors: vec![(at, nom::error::VerboseErrorKind::Context(what))],
    })
}

/// A line with nothing on it but padding.
///
/// KIF puts one before each `変化：` block, and R-KIF-002 lets one sit anywhere
/// in the move list. [`is_padding`] rather than `space0`, so that "nothing on
/// it" means the same here as everywhere else: a line holding one full-width
/// space is blank to a person reading it.
pub(super) fn blank_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    let (rest, _) = line_ending(input.trim_start_matches(is_padding))?;
    Ok((rest, ""))
}

/// A `#` line: a note from the program that wrote the file (R-KIF-002).
pub(super) fn program_comment_line(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    comment_line(input)
}

fn comment_line(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    preceded(
        take_while(is_padding),
        map(
            delimited(tag("#"), not_line_ending, end_of_line),
            String::from,
        ),
    )(input)
}

/// Whether a `|` or `+` line could still be a piece of this record's board.
///
/// Two facts decide it, and **neither can be worked out from the line in front
/// of the skip**: whether [`parse_without_moves`] came back with a diagram (a
/// record has one board and it is read there), and whether the reader has taken
/// its first move. Each is known in exactly one place, so this carries the
/// answer from there instead of letting every skip derive it again. Three
/// skips deriving it three ways disagree, and a `.ki2` then reads as an empty
/// 平手 with the position gone (GAP-007, D4).
///
/// There is no way to make one out of a `bool`: [`parse_without_moves`] hands
/// out the only ones there are, and [`Self::after`] is the only thing
/// that changes one. A reader that forgets to pass it on keeps guarding, which
/// costs a refused `|先手|後手|` (D1's side of the trade) rather than a
/// position that vanishes.
#[derive(Clone, Copy)]
pub(super) struct WhereABoardCouldBe(bool);

impl WhereABoardCouldBe {
    /// What is left of this after reading `run`.
    ///
    /// A run that read a move puts the opening block behind it: a `|先手|後手|`
    /// or `+123` below cannot be a board — the board is above the moves — and
    /// the format has always allowed such a line. A run of comments or a `まで…`
    /// line does not, so the diagram of a record that carries the last game's
    /// special move at its head is still a diagram.
    ///
    /// Takes the run rather than a `bool` so that "what counts as a move" is
    /// answered here and not once per caller — four callers with four answers is
    /// the shape this type exists to remove.
    pub(super) fn after(self, run: &[MoveFormat]) -> Self {
        if run.iter().any(|mf| mf.move_.is_some()) {
            Self(false)
        } else {
            self
        }
    }

    fn keeps_frame_line(self) -> bool {
        self.0
    }

    /// For tests that call a run parser directly, past the point where a board
    /// could be. The readers cannot reach this: what they get comes from
    /// [`parse_without_moves`], which is the only place that knows.
    #[cfg(test)]
    pub(super) fn past_a_board_for_tests() -> Self {
        Self(false)
    }
}

/// A line that is none of the shapes the move list is made of, skipped whole.
///
/// Asked past the indentation ([`padding`]), because what a line is does not
/// change with how far in it starts.
///
/// What it declines to start on is not every shape the reader knows — it is the
/// ones that mean something else at the head of a line: a line ending (a blank
/// line belongs to the caller), a numbered move line ([`opens_numbered_line`]
/// — shared, because a KI2 that holds one is a record something went wrong with
/// and the move must not go with the line), `*` and `&` (a comment and a bookmark,
/// R-KIF-010 / R-KIF-011), and [`SIDE_MARKS`] (a KI2 move). `#`, `変化：` and
/// `まで…` are **not** among them, so a caller that wants one of those read as
/// itself has to try it before this (`kif::skippable_line_above_the_main_line`).
///
/// `where_a_board_could_be` adds `|` and `+` to that list while a board could
/// still be arriving. They are the frame and the ranks of a diagram, which
/// reaches the reader through no other path (GAP-007), and a skip that takes one
/// takes the position with it — the record comes back as an empty 平手 and the
/// writer saves that over the original (D4). Past the first move, or below a
/// diagram that was read, the same two characters cannot be a board and the
/// format has always allowed them.
///
/// `&` is there because a bookmark is kept as a comment (R-KIF-011), and letting
/// this parser take the line instead drops it without a word — which is how a
/// bookmark at the head of a `変化：` block that `to_kif` had just written went
/// missing.
///
/// The line endings have to be excluded too. Without them the parser starts on
/// the newline of a *blank* line, takes the line after it as this line's
/// content, and swallows two lines where one was meant — so a blank line in the
/// middle of a record destroys the move that follows it.
pub(super) fn not_move_line(
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, &str, VerboseError<&str>> {
    // Asked past the indentation. What a line *is* does not change with how far
    // in it starts (R-KIF-008 writes its move lines three columns in), so a
    // reader that looks only at column 0 takes an indented `*` comment or
    // `まで…` for prose and throws it away without a word. A line ending is
    // still one of the shapes declined, so a blank line goes to `blank_line`.
    //
    // A predicate rather than `none_of`'s pattern, so that [`SIDE_MARKS`] can be
    // asked instead of spelled out again: a mark this knew and the table did not
    // would make a line one reader skips and the other keeps.
    let head = input.trim_start_matches(is_padding);
    satisfy(|c| {
        !matches!(c, '*' | '&')
            && !crate::notation::LINE_ENDS.contains(&c)
            && !SIDE_MARKS.iter().any(|(mark, _)| *mark == c)
    })(head)?;
    // `|` and `+` open the rows and the frame of a board diagram
    // (R-KIF-014 / D6), and a record with a board reaches the reader through no
    // other path (`research/90-gaps.md` GAP-007) — so skipping one of its lines
    // takes the whole position, and the writer saves an empty 平手 over the
    // original (D4). Left where the board could still be, the leftover-input
    // check names the line that could not be read (D1).
    //
    // Only there. The board is read by `parse_without_moves`, so once a move has
    // been read no `|` or `+` line can be a piece of one, and refusing them
    // buys nothing — it just drops `|先手|後手|` and `+123` from records the
    // format has always allowed.
    if where_a_board_could_be.keeps_frame_line() && head.starts_with(['|', '+']) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            ErrorKind::Not,
        )));
    }
    if opens_numbered_line(head) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            ErrorKind::Not,
        )));
    }
    terminated(not_line_ending, end_of_line)(input)
}

pub(super) fn move_comment_line(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    // Past the indentation, the same as [`not_move_line`]: those two divide
    // every line between them, so a column one of them looks past and the other
    // does not is a line that falls to the skip and is lost (R-KIF-010 /
    // R-KIF-011).
    preceded(
        take_while(is_padding),
        alt((
            map(
                delimited(tag("*"), not_line_ending, end_of_line),
                String::from,
            ),
            map(delimited(tag("&"), not_line_ending, end_of_line), |s| {
                String::from("&") + s
            }),
        )),
    )(input)
}

pub(super) fn piece_kind(input: &str) -> IResult<&str, Kind, VerboseError<&str>> {
    alt((
        value(Kind::FU, tag("歩")),
        value(Kind::KY, tag("香")),
        value(Kind::KE, tag("桂")),
        value(Kind::GI, tag("銀")),
        value(Kind::KI, tag("金")),
        value(Kind::KA, tag("角")),
        value(Kind::HI, tag("飛")),
        value(Kind::OU, alt((tag("玉"), tag("王")))),
        value(Kind::TO, tag("と")),
        value(Kind::NY, alt((tag("杏"), tag("成香")))),
        value(Kind::NK, alt((tag("圭"), tag("成桂")))),
        value(Kind::NG, alt((tag("全"), tag("成銀")))),
        value(Kind::UM, tag("馬")),
        value(Kind::RY, alt((tag("龍"), tag("竜")))),
    ))(input)
}

fn kansuji(input: &str) -> IResult<&str, u8, VerboseError<&str>> {
    alt((
        value(18, tag("十八")),
        value(17, tag("十七")),
        value(16, tag("十六")),
        value(15, tag("十五")),
        value(14, tag("十四")),
        value(13, tag("十三")),
        value(12, tag("十二")),
        value(11, tag("十一")),
        value(10, tag("十")),
        value(9, tag("九")),
        value(8, tag("八")),
        value(7, tag("七")),
        value(6, tag("六")),
        value(5, tag("五")),
        value(4, tag("四")),
        value(3, tag("三")),
        value(2, tag("二")),
        value(1, tag("一")),
    ))(input)
}

fn information_value_hand(input: &str) -> IResult<&str, Hand, VerboseError<&str>> {
    // The padding in front of the value belongs to the value, the same as the
    // padding behind it: `後手の持駒： 歩` is the value `歩`.
    let (input, _) = padding(input)?;
    alt((
        // The padding after the value belongs to the value, in every arm. Left
        // out of one of them, `後手の持駒：なし ` reaches the newline check with
        // a space still to read and the whole file is refused with an error
        // naming nothing (`in Tag:`) — while `後手の持駒：歩 ` reads. tsshogi
        // splits the value on spaces and drops the empty pieces (R-KIF-014), so
        // that record opens on the TS side of the consumer and not on this one.
        terminated(value(Hand::default(), tag("なし")), take_while(is_padding)),
        map_res(
            many1(terminated(
                pair(piece_kind, map(opt(kansuji), |o| o.unwrap_or(1))),
                take_while(is_padding),
            )),
            |v| {
                // The counts come from the file, so they can add up past what a
                // `u8` holds. Overflowing here would silently record a
                // different hand.
                v.iter().try_fold(Hand::default(), |mut acc, &(k, n)| {
                    let slot = match k {
                        Kind::FU => &mut acc.FU,
                        Kind::KY => &mut acc.KY,
                        Kind::KE => &mut acc.KE,
                        Kind::GI => &mut acc.GI,
                        Kind::KI => &mut acc.KI,
                        Kind::KA => &mut acc.KA,
                        Kind::HI => &mut acc.HI,
                        _ => return Err(()),
                    };
                    *slot = slot.checked_add(n).ok_or(())?;
                    Ok(acc)
                })
            },
        ),
        // Last, so that it answers only for a value the arms above found nothing
        // in. An empty value is an empty hand for the same reason a padded one
        // is: tsshogi's `readHand` skips a section that is empty exactly as it
        // skips `なし`. Writing `なし` is what R-KIF-014 asks of a *writer*; a
        // reader that refuses the blank one refuses the whole file, board and
        // all, over a line that says nothing.
        terminated(
            value(Hand::default(), take_while(is_padding)),
            peek(end_of_line),
        ),
    ))(input)
}

fn information_value_preset(input: &str) -> IResult<&str, Information<'_>, VerboseError<&str>> {
    // The padding in front of the value belongs to the value, the same as the
    // padding behind it and the padding around the key. Left out, `手合割： 香落ち`
    // falls to the key-value rule and the board is 平手 — where Black opens and
    // the upper hand does not (R-HC-001 / R-RULE-006), so every side in the game
    // is the wrong one. tsshogi's `readHandicap` trims the value.
    let (input, _) = padding(input)?;
    // Longest name first, or `香落ち` would swallow the tail of `右香落ち`.
    let named = crate::handicap::names_longest_first()
        .into_iter()
        .find_map(|handicap| {
            let (tail, _) = tag::<_, _, VerboseError<&str>>(handicap.kif_name)(input).ok()?;
            Some((tail, handicap.preset))
        });
    let (rest, preset) = match named {
        Some(found) => found,
        None => {
            let (tail, _) = tag(crate::handicap::OTHER_NAME)(input)?;
            (tail, Preset::PresetOther)
        }
    };
    // The padding after the name belongs to the name. `手合割：香落ち\t` left it
    // to the key-value rule instead, and a handicap filed under `header` is a
    // handicap the board never sees: the record falls back to 平手, where Black
    // opens and the upper hand does not (R-HC-001 / R-RULE-006), so every side
    // in the game is the wrong one.
    let (rest, _) = take_while(is_padding)(rest)?;
    Ok((rest, Information::Preset(preset)))
}

/// A `手合割：<名前>` line naming one of the handicaps in `40-handicap.md`.
///
/// Anything else on the line — a name this table has no entry for, a handicap
/// followed by a note — is not this line, and the reader hands it to
/// [`information_line_keyvalue`] to keep as text (R-KIF-004).
///
/// The text is kept, but the position it named is not: `initial` falls back to
/// the even game, and for a handicap that also flips every move's side, since
/// the upper hand moves first only in a handicap (R-HC-001). `手合割：香落ち（30分）`
/// reads as a hirate game with Black to open (`research/90-gaps.md` GAP-021).
/// Being strict here instead would refuse the file outright, which is worse and
/// still not the fix — the fix is a table with an entry for what the file says.
fn information_line_preset(input: &str) -> IResult<&str, Information<'_>, VerboseError<&str>> {
    terminated(
        preceded(
            tuple((padding, tag(crate::handicap::KIF_KEYWORD), padding, colon)),
            information_value_preset,
        ),
        end_of_line,
    )(input)
}

fn information_line_hands(input: &str) -> IResult<&str, Information<'_>, VerboseError<&str>> {
    let line = input;
    let (rest, color) = delimited(
        padding,
        alt((
            value(Color::Black, tag("先手")),
            value(Color::White, tag("後手")),
            value(Color::Black, tag("下手")),
            value(Color::White, tag("上手")),
        )),
        tuple((tag("の持駒"), padding, colon)),
    )(input)?;
    // Past the prefix this line states a hand, whatever follows. Reporting a
    // recoverable error would send it to `information_line_keyvalue`, which
    // files the whole line under `header` and leaves the hand empty — including
    // the pieces written *before* the one that could not be read. A later drop
    // from that hand then fails to normalize, and the message names the move
    // rather than the line that actually broke.
    let fail = |_| broken_line(line, "this hand line cannot be read");
    let (rest, hand) = information_value_hand(rest).map_err(fail)?;
    let (rest, _) = end_of_line(rest).map_err(fail)?;
    Ok((
        rest,
        match color {
            Color::Black => Information::HandBlack(hand, line),
            Color::White => Information::HandWhite(hand, line),
        },
    ))
}

/// Whether `key` is one of the words that name a line of the opening block, and
/// so may be written with either colon ([`colon`]).
fn names_line(key: &str) -> bool {
    key == crate::handicap::KIF_KEYWORD || key.ends_with("の持駒")
}

/// A `<キーワード>：<値>` line, which a header can be any of (R-KIF-004).
///
/// This is where every header line that is nothing more specific ends up, so it
/// is also where a header that swallowed the line under it has to be caught.
/// `carries_a_line` is the reader's own answer to "is this value a line of my
/// format?", because the header block is shared and the answer is not: a KI2
/// whose `手合割：平手` lost its newline files the entire game under
/// `header["手合割"]` and comes back `Ok` with no moves in it, while a KIF move
/// line joined to a header is the same shape as `棋戦：第 3 回` and cannot be
/// told from it (`research/90-gaps.md` GAP-020).
///
/// A comment (R-KIF-010) and a bookmark (R-KIF-011) are lines of their own, and
/// either can hold a `：` — `*（主催：新聞三社連合）` is the opening line of a real
/// record. Read as a header the line takes its text with it and leaves a key
/// nobody wrote (`*主催`). Nothing puts them back: what stops the header block in
/// KIF is the `手数----指手---------消費時間--` line, which R-KIF-012 says a
/// record need not have, and which KI2 has no equivalent of at all.
fn information_line_keyvalue<'a>(
    shapes: LineShapes,
) -> impl FnMut(&'a str) -> IResult<&'a str, Information<'a>, VerboseError<&'a str>> {
    move |input| {
        // Belt and braces: `information_lines` reads these as comments before it
        // gets here, and a reordering of that `alt` would otherwise put them
        // back into `header` without a word.
        if input.trim_start_matches(is_padding).starts_with(['*', '&']) {
            return Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                ErrorKind::Not,
            )));
        }
        // The full-width rule first, so that a key holding a half-width colon
        // (`備考:あり：なし`) is still split at the colon the format uses. Asking
        // the wider question first cuts such a key short, and the line then
        // looks like "an unknown key with a half-width colon" and is dropped —
        // including lines this crate's own writer produces from a `header` the
        // consumer filled in.
        // Both keys run up to a colon or the end of the line, and both take the
        // sets that name those — a third colon added to [`COLONS`] has to reach
        // the key rule too, or it is swallowed into the key and the line is
        // dropped with nothing left for [`colon`] to find.
        let up_to_the_colon = |c: char| c != COLON && !crate::notation::LINE_ENDS.contains(&c);
        let up_to_either =
            |c: char| !COLONS.contains(c) && !crate::notation::LINE_ENDS.contains(&c);
        let (rest, key, mark) = match preceded(
            padding,
            terminated(take_while1(up_to_the_colon), char(COLON)),
        )(input)
        {
            Ok((rest, key)) => (rest, key, COLON),
            Err(_) => {
                let (rest, key) = preceded(padding, take_while1(up_to_either))(input)?;
                let (rest, mark) = colon(rest)?;
                (rest, key, mark)
            }
        };
        // The padding on either side of the key belongs to the line, not to the
        // key. `手合割 ：香落ち` filed under `header["手合割 "]` is a handicap the
        // board never sees, and D16's `contains_key` misses it too — the record
        // reads as 平手, every side is reversed (R-HC-001 / R-RULE-006), and
        // writing it back produces two `手合割` lines.
        let key = key.trim_end_matches(is_padding);
        // Half-width only where the key is one this reader knows. A key can be
        // anything (R-KIF-004), so taking a half-width colon from every key
        // makes a header line out of every `key: value` in any text —
        // `{"header":{},"moves":[{}]}` comes back as a kifu with one header, and
        // the error D1 and D8 exist to raise reaches nobody.
        //
        // The known keys need it because their own rules already take it
        // (`colon`) and then decline when the value is not one this crate has a
        // board for: `手合割:詰将棋` would otherwise land nowhere and fall to the
        // prose skip, losing even the word the file wrote. GAP-021 keeps such a
        // value under `header`, which is where this puts it.
        if mark == ':' && !names_line(key) {
            return Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                ErrorKind::OneOf,
            )));
        }
        let (rest, value) = terminated(not_line_ending, end_of_line)(rest)?;
        if (shapes.carries_a_line)(value) {
            return Err(broken_line(input, "this line runs into the one below it"));
        }
        Ok((
            rest,
            Information::KeyValue(key.to_owned(), value.to_owned()),
        ))
    }
}

fn informations(
    shapes: LineShapes,
    input: &str,
) -> IResult<&str, InformationData<'_>, VerboseError<&str>> {
    information_lines(shapes, input)
}

fn information_lines<'a>(
    shapes: LineShapes,
    input: &'a str,
) -> IResult<&'a str, InformationData<'a>, VerboseError<&'a str>> {
    let (rest, read) = many0(preceded(
        many0(comment_line),
        alt((
            // Before the key-value rule, which would file the line under a key
            // nobody wrote, and *as one of the alternatives* rather than as a
            // guard: a guard that refuses ends `many0`, and everything below the
            // comment — the rest of the header, the board, the `手合割` — is then
            // read by nobody (R-KIF-004, R-HC-001).
            map(move_comment_line, Information::Comment),
            information_line_preset,
            information_line_hands,
            information_line_keyvalue(shapes),
        )),
    ))(input)?;
    let mut acc = InformationData::default();
    for info in read {
        match info {
            Information::Preset(p) => acc.preset = Some(p),
            Information::HandBlack(h, line) => acc.state_a_hand(Color::Black, h, line)?,
            Information::HandWhite(h, line) => acc.state_a_hand(Color::White, h, line)?,
            Information::KeyValue(k, v) => {
                acc.map.insert(k, v);
            }
            Information::Comment(c) => acc.comments.push(c),
        }
    }
    Ok((rest, acc))
}

fn board_piece_color(input: &str) -> IResult<&str, Color, VerboseError<&str>> {
    alt((value(Color::Black, tag(" ")), value(Color::White, tag("v"))))(input)
}

fn board_piece(input: &str) -> IResult<&str, Piece, VerboseError<&str>> {
    alt((
        value(Piece::empty(), tag(" ・")),
        map(pair(board_piece_color, piece_kind), |(c, k)| Piece {
            color: Some(c),
            kind: Some(k),
        }),
    ))(input)
}

fn board_row(input: &str) -> IResult<&str, Vec<Piece>, VerboseError<&str>> {
    terminated(
        delimited(
            // Only in front of the frame. Inside a row a space is data — it is
            // how `board_piece_color` spells Black (R-KIF-014 / D6).
            pair(padding, tag("|")),
            count(board_piece, 9),
            preceded(tag("|"), one_of("一二三四五六七八九")),
        ),
        pair(padding, end_of_line),
    )(input)
}

/// Whether a board diagram could open on this line: the file numbers, the
/// frame that sits under them (R-KIF-014 / D6), or a rank of a diagram written
/// without either (GAP-039).
///
/// Either alone says a diagram starts here — that is what lets [`board`] name
/// the one of the two that is misspelled instead of unwinding to prose.
fn opens_board(head: &str) -> bool {
    head.starts_with("９ ８ ７ ６ ５ ４ ３ ２ １")
        || head.starts_with("+---")
        || head.starts_with('|')
}

fn board(input: &str) -> IResult<&str, [[Piece; 9]; 9], VerboseError<&str>> {
    // A diagram may come as its nine ranks alone, without the file numbers and
    // the frame (tsshogi's tests write one; GAP-039). R-KIF-014 has no primary
    // source for the frame either. A rank is a line no other shape starts with
    // `|`, so its first one is enough to say a diagram starts here.
    if board_row(input).is_ok() {
        let (rest, ranks) = board_ranks(input)?;
        let (rest, _) = opt(delimited(
            padding,
            tag("+---------------------------+"),
            pair(padding, end_of_line),
        ))(rest)?;
        return Ok((rest, ranks));
    }
    // The two columns in front of `９` line the file numbers up with the frame
    // below them, so they are padding and not part of the word.
    let numbers = delimited(
        padding,
        tag("９ ８ ７ ６ ５ ４ ３ ２ １"),
        pair(padding, line_ending),
    )(input);
    let frame = |input| {
        delimited(
            padding,
            tag("+---------------------------+"),
            pair(padding, line_ending),
        )(input)
    };
    // Either line is enough to say a diagram starts here, so a record that has
    // one of them and not the other is a broken diagram rather than prose. Both
    // recoverable, the whole diagram unwinds and the message points at its first
    // line — the one line known to be intact — while the reader looking for the
    // fault is nine lines further down (D1).
    let rest = match numbers {
        Ok((rest, _)) => match frame(rest) {
            Ok((rest, _)) => rest,
            Err(_) => return Err(broken_line(rest, "the board has no frame under its files")),
        },
        Err(err) => {
            // The line may be the file numbers, spelled wrong. What says so is
            // what follows it: a frame and then a rank.
            if let Some((_, below)) = input.split_once('\n') {
                if frame(below).is_ok_and(|(rest, _)| board_row(rest).is_ok()) {
                    return Err(broken_line(
                        input,
                        "the board's file numbers cannot be read",
                    ));
                }
            }
            let (rest, _) = frame(input)?;
            match board_row(rest) {
                Ok(_) => return Err(broken_line(input, "the board has no file numbers")),
                // A `+---` line with no rank under it opens no diagram —
                // `+7776FU` and the like are lines records have always carried.
                Err(_) => return Err(err),
            }
        }
    };
    let (rest, ret) = board_ranks(rest)?;
    let (rest, _) = delimited(
        padding,
        tag("+---------------------------+"),
        pair(padding, end_of_line),
    )(rest)
    .map_err(|_| broken_line(rest, "the board has no frame under its last rank"))?;
    Ok((rest, ret))
}

/// The nine ranks of a board diagram, top to bottom.
///
/// Called where the input has already said it is a diagram, so a rank that
/// cannot be read names itself. A recoverable error here would unwind the whole
/// diagram and the message would come out pointing at its first line — the one
/// line known to be intact. D1 asks for the line that broke, and the reader is
/// someone looking for it in their file.
fn board_ranks(mut rest: &str) -> IResult<&str, [[Piece; 9]; 9], VerboseError<&str>> {
    let mut rows = Vec::with_capacity(9);
    while rows.len() < 9 {
        let (tail, row) = board_row(rest).map_err(|_| {
            // Nothing left is not a rank that cannot be read: pointing at it
            // names the line after the last one, which the file does not have.
            if rest.trim_start_matches(is_padding).is_empty() {
                broken_line(rest, "the file ends inside the board")
            } else {
                broken_line(rest, "this rank of the board cannot be read")
            }
        })?;
        rows.push(row);
        rest = tail;
    }
    let mut ret = [[Piece::empty(); 9]; 9];
    for (rank, row) in rows.into_iter().enumerate() {
        for (file, piece) in row.into_iter().enumerate() {
            ret[8 - file][rank] = piece;
        }
    }
    Ok((rest, ret))
}

fn place_x(input: &str) -> IResult<&str, u8, VerboseError<&str>> {
    alt((
        value(1, tag("１")),
        value(2, tag("２")),
        value(3, tag("３")),
        value(4, tag("４")),
        value(5, tag("５")),
        value(6, tag("６")),
        value(7, tag("７")),
        value(8, tag("８")),
        value(9, tag("９")),
    ))(input)
}

fn place_y(input: &str) -> IResult<&str, u8, VerboseError<&str>> {
    alt((
        value(1, tag("一")),
        value(2, tag("二")),
        value(3, tag("三")),
        value(4, tag("四")),
        value(5, tag("五")),
        value(6, tag("六")),
        value(7, tag("七")),
        value(8, tag("八")),
        value(9, tag("九")),
    ))(input)
}

pub(super) fn move_to(input: &str) -> IResult<&str, Option<PlaceFormat>, VerboseError<&str>> {
    alt((
        // The padding after `同` is padding, however much of it there is.
        // R-NOT-002 says a full-width space usually follows and sometimes does
        // not; nothing says "exactly one", and tsshogi's `moveRegExp` writes
        // `同　*`.
        // A file whose columns were re-aligned by a word processor has two.
        value(None, terminated(tag("同"), take_while(is_padding))),
        map(pair(place_x, place_y), |(x, y)| Some(PlaceFormat { x, y })),
        // GAP-037: the square in Arabic digits — `76歩` as tsshogi's tests
        // write it, `７６歩` as the 日本将棋連盟 records it (R-NOT-001 note).
        map(
            pair(arabic_digit, alt((place_y, arabic_digit))),
            |(x, y)| Some(PlaceFormat { x, y }),
        ),
    ))(input)
}

/// A digit 1-9, half-width or full-width.
fn arabic_digit(input: &str) -> IResult<&str, u8, VerboseError<&str>> {
    let mut chars = input.chars();
    let digit = chars.next().and_then(|c| match c {
        '1'..='9' => Some(c as u8 - b'0'),
        '１'..='９' => Some((c as u32 - '０' as u32) as u8),
        _ => None,
    });
    match digit {
        Some(d) => Ok((chars.as_str(), d)),
        None => Err(nom::Err::Error(VerboseError {
            errors: vec![(input, nom::error::VerboseErrorKind::Context("a digit 1-9"))],
        })),
    }
}

/// The `後手番` line that says the position starts with White to move
/// (R-KIF-014).
///
/// The word owns the rest of the line ([`ends_here`]). This parser sits under an
/// `opt`, so a line it merely declines is read as one the format has no shape
/// for and skipped — taking a `後手番` joined with the moves under it with it,
/// and leaving a tsume that starts from the wrong side and has no moves.
fn side_to_move_line(
    shapes: LineShapes,
) -> impl FnMut(&str) -> IResult<&str, Option<Color>, VerboseError<&str>> {
    move |input| {
        opt(|input| {
            let (rest, color) = preceded(
                padding,
                alt((
                    value(Color::Black, tag("先手番")),
                    value(Color::White, tag("後手番")),
                    value(Color::Black, tag("下手番")),
                    value(Color::White, tag("上手番")),
                )),
            )(input)?;
            let (rest, _) = ends_here(shapes, input, rest)?;
            Ok((rest, color))
        })(input)
    }
}

/// What the header block leaves behind: the record so far, the comments that
/// stood among its lines, whether a board could still be arriving, and whether
/// any line of the block itself was read.
pub(super) type HeaderBlock<'a> = (JsonKifuFormat, Vec<String>, WhereABoardCouldBe, bool);

/// Reads the header block: everything above the moves, including the board.
///
/// `shapes` is how the reader says what its own lines look like; see
/// [`LineShapes`]. The block itself is the same in KIF and KI2, and the shapes
/// are the only thing that differs inside it.
///
/// Returns the comments that stood among those lines as well — see
/// [`comments_on_the_initial_position`].
///
/// The third of what it hands back is [`WhereABoardCouldBe`] — this is the one
/// place that knows whether the record's diagram was read, and the skips below
/// have no way of finding out for themselves (GAP-007).
///
/// Fails where a hand **holding pieces** was stated and no board came: they have
/// nowhere to sit, and a record that keeps neither is not the record that was
/// written (D1, D4). An empty hand states nothing the preset does not already
/// say, so it is read like any other header line.
pub(super) fn parse_without_moves(
    shapes: LineShapes,
    input: &str,
) -> IResult<&str, HeaderBlock<'_>, VerboseError<&str>> {
    // Blank lines and prose do not end the block. A `many0` that stops at the
    // first line none of its arms take makes the *previous* line decide who owns
    // the next one: everything below — the rest of the header, the board, the
    // 手合割, the 後手番 — then goes to the move list's skip, which takes it
    // whole and hands back a record with the position gone (D1, D21, GAP-007).
    //
    // What may be skipped here is only what is not a line of this format at all
    // (`shapes.opens_line` decides, so the move list still gets its lines) and
    // not a piece of a board (`not_move_line` with the guard on, since the board
    // may still be arriving).
    fn skippable(shapes: LineShapes, input: &str) -> IResult<&str, &str, VerboseError<&str>> {
        // Not a line of this format, and not a line a board diagram opens
        // with: a diagram whose first line is misspelled is a broken diagram,
        // and skipping that line leaves the reader naming the frame below it
        // (D1 asks for the line that broke).
        let head = input.trim_start_matches(is_padding);
        if (shapes.opens_line)(head) || opens_board(head) {
            return Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                ErrorKind::Not,
            )));
        }
        not_move_line(WhereABoardCouldBe(true), input)
    }
    let mut rest = input;
    let mut info = InformationData::default();
    let mut opt_board = None;
    let mut side_to_move = None;
    let mut read_a_line_of_its_own = false;
    loop {
        let (after, read) = informations(shapes, rest)?;
        read_a_line_of_its_own |= after.len() < rest.len();
        info.absorb(read)?;
        rest = after;
        if opt_board.is_none() {
            // A `Failure` here is the diagram saying it is one and is broken
            // (D21's three answers). Swallowing it hands the record to the skip,
            // which cannot take the diagram either, and the leftover check then
            // names the frame below the line that actually broke.
            match board(rest) {
                Ok((after, read)) => {
                    opt_board = Some(read);
                    read_a_line_of_its_own = true;
                    rest = after;
                    continue;
                }
                Err(err @ nom::Err::Failure(_)) => return Err(err),
                Err(_) => {}
            }
        }
        if side_to_move.is_none() {
            if let Ok((after, Some(read))) = side_to_move_line(shapes)(rest) {
                side_to_move = Some(read);
                read_a_line_of_its_own = true;
                rest = after;
                continue;
            }
        }
        match alt((blank_line, |input| skippable(shapes, input)))(rest) {
            Ok((after, _)) => rest = after,
            Err(nom::Err::Failure(err)) => return Err(nom::Err::Failure(err)),
            Err(_) => break,
        }
    }
    let input = rest;
    let has_a_board = opt_board.is_some();
    // A hand belongs to a board. Without one there is nowhere to put it, and
    // `initial.data` comes back `None` with the pieces gone — a 詰将棋 whose
    // diagram could not be read comes back as an empty 平手, and the writer
    // saves that over the original (D4, GAP-007).
    //
    // Only where something is actually lost. `先手の持駒：なし` above a
    // `手合割` line states nothing the preset does not already say, and refusing
    // it would drop records tsshogi opens — R-KIF-014 warns about exactly that
    // disagreement, since the board block is a convention and not in the spec.
    if let Some(line) = info.hand_line_without_board().filter(|_| !has_a_board) {
        return Err(broken_line(
            line,
            "this record states a hand but has no board for it",
        ));
    }
    let initial = if let Some(board) = opt_board {
        Some(Initial {
            preset: Preset::PresetOther,
            data: Some(StateFormat {
                color: side_to_move.unwrap_or(Color::Black),
                board,
                hands: info.hands(),
            }),
        })
    } else {
        Some(Initial {
            preset: info.preset.unwrap_or(Preset::PresetHirate),
            data: None,
        })
    };
    Ok((
        input,
        (
            JsonKifuFormat {
                header: info.map,
                initial,
                moves: Vec::new(),
            },
            // R-KIF-010: a comment above the first move belongs to the starting
            // position. The caller owns `moves`, so it puts them there.
            info.comments,
            // A record has one board. If it was read here, nothing below is part
            // of one; if it was not, a `|` or `+` line below is a board this
            // reader could not take.
            WhereABoardCouldBe(!has_a_board),
            // Whether a line of the block itself was read. Bytes consumed is not
            // the same question now that prose is skipped here: a file that is
            // not a kifu at all would answer yes, and the empty record it comes
            // back as is what the consumer picks an encoding on
            // (`is_unrecognised`, D8).
            read_a_line_of_its_own,
        ),
    ))
}

/// Puts the comments that stood among the header lines in front of the ones the
/// move reader found above the first move — they are all comments on the
/// initial position (R-KIF-010), and the file's order is the order they were
/// written in.
pub(super) fn comments_on_the_initial_position(
    from_the_header: Vec<String>,
    moves: &mut [MoveFormat],
) {
    if from_the_header.is_empty() {
        return;
    }
    let Some(first) = moves.first_mut() else {
        return;
    };
    let rest = first.comments.take().unwrap_or_default();
    first.comments = Some(from_the_header.into_iter().chain(rest).collect());
}

#[cfg(test)]
mod tests;
