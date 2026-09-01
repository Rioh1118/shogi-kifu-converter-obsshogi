use crate::jkf::*;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag, take_while};
use nom::character::complete::{line_ending, not_line_ending, one_of, satisfy};
use nom::combinator::{eof, map, map_res, opt, peek, value};
use nom::error::{ErrorKind, ParseError, VerboseError};
use nom::multi::{count, many0, many1};
use nom::sequence::{delimited, pair, preceded, terminated, tuple};
use nom::IResult;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Information {
    Preset(Preset),
    HandBlack(Hand),
    HandWhite(Hand),
    KeyValue(String, String),
    /// A comment or a bookmark standing among the header lines
    /// (R-KIF-010 / R-KIF-011).
    Comment(String),
}

#[derive(Debug, Default, PartialEq, Eq)]
struct InformationData {
    preset: Option<Preset>,
    hands: [Hand; 2],
    map: HashMap<String, String>,
    comments: Vec<String>,
}

impl InformationData {
    fn merged(lhs: Self, rhs: Self) -> InformationData {
        InformationData {
            preset: lhs.preset.or(rhs.preset),
            hands: Self::merged_hands(lhs.hands, rhs.hands),
            map: lhs.map.into_iter().chain(rhs.map).collect(),
            comments: lhs.comments.into_iter().chain(rhs.comments).collect(),
        }
    }
    fn merged_hands(lhs: [Hand; 2], rhs: [Hand; 2]) -> [Hand; 2] {
        [
            Self::merged_hand(lhs[0], rhs[0]),
            Self::merged_hand(lhs[1], rhs[1]),
        ]
    }
    /// Adds the hands stated before and after the board.
    ///
    /// Saturating rather than wrapping: a file that states a hand twice is
    /// broken either way, and a wrapped count would look like a real one.
    fn merged_hand(lhs: Hand, rhs: Hand) -> Hand {
        Hand {
            FU: lhs.FU.saturating_add(rhs.FU),
            KY: lhs.KY.saturating_add(rhs.KY),
            KE: lhs.KE.saturating_add(rhs.KE),
            GI: lhs.GI.saturating_add(rhs.GI),
            KI: lhs.KI.saturating_add(rhs.KI),
            KA: lhs.KA.saturating_add(rhs.KA),
            HI: lhs.HI.saturating_add(rhs.HI),
        }
    }
}

/// Ends a line, at the end of the file as well as at a newline.
///
/// A text file need not end with a newline, and kifu written by hand and by
/// other software both turn up without one. Requiring `line_ending` drops the
/// last line — a move, a comment or the `まで<N>手で…` — and says nothing about
/// it. Every line parser here is anchored on something it must consume first,
/// so accepting the empty match at the end cannot loop.
pub(super) fn end_of_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
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
/// skipping such a line where it turned out to be prose is the trade D1 takes
/// (`parser::tests::a_record_the_reader_stops_in_the_middle_of_is_an_error`).
///
/// A line that merely holds one somewhere — `※▲２六歩が本筋` — is prose to KIF
/// and is skipped, while `ki2::a_line_only_prose_opens` asks `contains` and
/// refuses the whole file. That asymmetry is `research/90-gaps.md` GAP-029,
/// which is waiting on a decision; do not read this table as a promise that KIF
/// keeps such a line.
///
/// The variants R-NOT-001 also lists (`☗`/`☖`, `⛊`/`⛉`, `▼`/`▽`) are not read yet:
/// `research/90-gaps.md` GAP-024, which names every place that has to learn a
/// new one together — the KI2 writer among them.
pub(super) const SIDE_MARKS: [(char, Color); 2] = [('▲', Color::Black), ('△', Color::White)];

/// Whether `c` is padding — space a line can carry without saying anything.
///
/// A predicate rather than a table of the three characters a keyboard makes
/// easily. KI2 is a record people read (R-KI2-001) and paste from wherever they
/// read it, and a web page pads with `\u{a0}` and `\u{2009}` as readily as an
/// editor pads with a tab. A reader whose idea of padding is narrower than the
/// writer's reads the padding as content, and then says the record is wrong
/// about something it is right about — `まで2手で投了\u{a0}` came back as
/// "this outcome is not one of the words KI2 has", naming `投了`, which is one
/// of them.
///
/// Line endings are not padding. Whatever else this takes, it must not take
/// those: [`begins_the_line_below`] steps over one character looking for the
/// newline that was lost, and a newline that is still there was never lost.
pub(super) fn is_padding(c: char) -> bool {
    c.is_whitespace() && !crate::notation::LINE_ENDS.contains(&c)
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
    one_of("：:")(input)
}

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
    pub(super) opens_a_line: fn(&str) -> bool,
    /// Whether the line is one this format writes its moves on.
    ///
    /// Asked by [`not_move_line`], which must not skip a move line. KIF numbers
    /// them, KI2 opens them with a mark ([`SIDE_MARKS`], which that skip looks
    /// for on its own) — so "starts with a digit" is a KIF shape and not a
    /// shared one, and a KIF note like `　1図以下` is prose either way.
    pub(super) opens_a_move_line: fn(&str) -> bool,
}

/// What a note opens with.
///
/// Prose about a move puts one of these in front of it — `※▲２六歩が本筋`,
/// `（まで先手良し）`, `【変化】` — which is exactly where
/// [`begins_the_line_below`] would otherwise look for the newline that was
/// lost. Whatever replaced a newline, it is not one of these.
pub(super) const NOTE_MARKERS: [char; 8] = ['※', '（', '(', '【', '[', '「', '〈', '＜'];

/// The shapes both formats share: a comment, a bookmark, a `#` note, a `変化：`
/// header, a `まで…` outcome.
pub(super) fn opens_a_shared_line(head: &str) -> bool {
    head.starts_with(['*', '&', '#']) || opens_a_branch_header(head) || head.starts_with("まで")
}

/// Whether `head` is the beginning of a `変化：<N>手` header.
///
/// The number is what makes it one. `変化：` on its own is two characters a
/// sentence can open with.
///
/// Built on [`branch_header_ply`] rather than beside it, so that what a reader
/// *counts* as a branch header and what it can *consume* are the same set by
/// construction. Held apart they drift, and the drift shows up in opposite
/// directions: a count wider than the read makes a branch that reads as the
/// main line carrying on (R-JKF-004), and a read wider than the count refuses
/// records the format has always allowed (D17).
pub(super) fn opens_a_branch_header(head: &str) -> bool {
    // `starts_with` first because this runs on nearly every line and
    // `branch_header_ply` builds a `VerboseError` on each miss — the cost
    // `kif::skip_interruptions` is hand-written to avoid.
    head.trim_start_matches(is_padding).starts_with("変化") && branch_header_ply(head).is_ok()
}

/// Whether a `変化：<N>手` line says nothing but that.
///
/// Not what makes it a header — a number followed by a word is still a branch
/// declaration, and tsshogi reads it as one (`変化：3手目`). What it decides is
/// whether an **empty** block under it is worth reporting: `変化：2手` with
/// nothing beneath it is a branch that went missing and D1 says so, while
/// `変化：2手を参照` with nothing beneath it is a note about a branch and there
/// was never anything to lose. D17's table already fixes the second spelling as
/// an annotation where it lands at the end of a line.
pub(super) fn a_branch_header_is_all_the_line_says(head: &str) -> bool {
    matches!(branch_header_ply(head), Ok((rest, _)) if {
        rest.starts_with(|c: char| {
            is_padding(c) || crate::notation::LINE_ENDS.contains(&c) || NOTE_MARKERS.contains(&c)
        }) || rest.is_empty()
    })
}

/// Reads `変化：<N>手` and returns `N`.
///
/// The same shape [`opens_a_branch_header`] recognises, so that a spelling one
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
/// all, and KI2 hands it to `attach_branch`, which asks whether a node is there
/// and finds none. Refusing a record over a digit nobody reads is exactly the
/// difference this function exists to close. (Real records need three digits;
/// the ceiling is here so that a damaged one is not fatal, not to support it.)
///
/// Starts past the indentation, so that a `変化：` one column in is the same
/// line as one at column 0 ([`padding`]). Every caller asks the question that
/// way, and the one that consumed it did not — which left a `変化:` above the
/// first move of a KI2 to be read as a branch of a main line that has no node
/// to hang it on.
///
/// Ply 0 is read here and refused by the reader that uses it. Plies count from 1
/// (R-JKF-001), so `変化：０手` names no move for a branch to be an alternative
/// to — but only KI2 looks at the number (D3, D19), and refusing the line here
/// would reject KIF records over a digit KIF never reads. See
/// `ki2::branch_header`.
pub(super) fn branch_header_ply(input: &str) -> IResult<&str, usize, VerboseError<&str>> {
    let unreadable = || nom::Err::Error(VerboseError::from_error_kind(input, ErrorKind::Digit));
    let (rest, _) = preceded(pair(padding, tag("変化")), colon)(input)?;
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
    // a reader — tsshogi's `branchRegExp` reads the number and stops.
    let (rest, _) = opt(tag("手"))(&rest[taken..])?;
    Ok((rest, ply))
}

/// Whether `head` is a `変化：<N>手` header and nothing else, to the end of its
/// line.
///
/// Asked where a line has already ended, not where one begins. `変化：2手` in
/// the annotation column of a move line — `  83 投了    変化：2手  ( 0:00)` —
/// is prose about a branch; reading it as the header of one says the record
/// runs into the line below it, which it does not, and refuses the whole file
/// over a note (D17).
fn a_branch_header_fills_the_line(head: &str) -> bool {
    match branch_header_ply(head) {
        Ok((rest, _)) => end_of_line(rest.trim_start_matches(is_padding)).is_ok(),
        Err(_) => false,
    }
}

/// Whether `head` is the beginning of a `<手数> <指し手>` line
/// (R-KIF-005 / R-KIF-008).
///
/// The number on its own is not the shape. A `( 0:01)` this reader has no shape
/// for, a bare `55`, and a note that opens `1図以下、先手優勢` all carry digits
/// and none of them is a line — what makes one is a number, then padding, then
/// something for the number to be about.
///
/// Shared, though only KIF writes such a line: a KI2 that holds one is a record
/// something went wrong with, and skipping it as prose would take the move with
/// it (D1).
pub(super) fn opens_a_numbered_line(head: &str) -> bool {
    let after_digits = head.trim_start_matches(|c: char| c.is_ascii_digit());
    after_digits.len() < head.len()
        && after_digits.starts_with(is_padding)
        && !after_digits.trim_start_matches(is_padding).is_empty()
}

/// Whether `tail` — what is left of a line the reader has finished with —
/// begins the line that should have been underneath it.
///
/// `shapes.opens_a_line` asks the same question of text that starts where a line
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
fn begins_the_line_below(shapes: LineShapes, tail: &str) -> bool {
    // A `変化：` here has to be the whole of the line it would begin. Everywhere
    // else the question is asked at the head of a line, where a header followed
    // by a note is still a header; here it is asked past the end of one, where
    // the same characters are the note.
    let opens = |head: &str| {
        if opens_a_branch_header(head) {
            return a_branch_header_fills_the_line(head);
        }
        (shapes.opens_a_line)(head)
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
    if begins_the_line_below(shapes, rest) {
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

/// A line that is none of the shapes the move list is made of, skipped whole.
///
/// Asked past the indentation ([`padding`]), because what a line is does not
/// change with how far in it starts.
///
/// What it declines to start on is not every shape the reader knows — it is the
/// ones that mean something else at the head of a line: a line ending (a blank
/// line belongs to the caller), a move line of this format
/// ([`LineShapes::opens_a_move_line`]), `*` and `&` (a comment and a bookmark,
/// R-KIF-010 / R-KIF-011), and [`SIDE_MARKS`] (a KI2 move). `#`, `変化：` and
/// `まで…` are **not** among them, so a caller that wants one of those read as
/// itself has to try it before this (`kif::skippable_line`).
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
    shapes: LineShapes,
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
        // `|` and `+` open the rows and the frame of a board diagram
        // (R-KIF-014 / D6). A record with a board reaches the reader through no
        // other path (`research/90-gaps.md` GAP-007), so skipping one of its
        // lines takes the whole position — the reader comes back with an empty
        // 平手 and the writer saves that over the original (D4). Left here, the
        // leftover-input check names the line that could not be read (D1).
        !matches!(c, '*' | '&' | '|' | '+')
            && !crate::notation::LINE_ENDS.contains(&c)
            && !SIDE_MARKS.iter().any(|(mark, _)| *mark == c)
    })(head)?;
    if (shapes.opens_a_move_line)(head) {
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
    // The padding in front of the value belongs to the value too. Taking it in
    // every arm but this one is what left `後手の持駒： 歩` refused while
    // `後手の持駒：歩 ` and `後手の持駒：` both read.
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
            peek(line_ending),
        ),
    ))(input)
}

fn information_value_preset(input: &str) -> IResult<&str, Information, VerboseError<&str>> {
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
fn information_line_preset(input: &str) -> IResult<&str, Information, VerboseError<&str>> {
    terminated(
        preceded(
            tuple((padding, tag(crate::handicap::KIF_KEYWORD), colon)),
            information_value_preset,
        ),
        line_ending,
    )(input)
}

fn information_line_hands(input: &str) -> IResult<&str, Information, VerboseError<&str>> {
    let line = input;
    let (rest, color) = delimited(
        padding,
        alt((
            value(Color::Black, tag("先手")),
            value(Color::White, tag("後手")),
            value(Color::Black, tag("下手")),
            value(Color::White, tag("上手")),
        )),
        pair(tag("の持駒"), colon),
    )(input)?;
    // Past the prefix this line states a hand, whatever follows. Reporting a
    // recoverable error would send it to `information_line_keyvalue`, which
    // files the whole line under `header` and leaves the hand empty — including
    // the pieces written *before* the one that could not be read. A later drop
    // from that hand then fails to normalize, and the message names the move
    // rather than the line that actually broke.
    let fail = |_| broken_line(line, "this hand line cannot be read");
    let (rest, hand) = information_value_hand(rest).map_err(fail)?;
    let (rest, _) = line_ending(rest).map_err(fail)?;
    Ok((
        rest,
        match color {
            Color::Black => Information::HandBlack(hand),
            Color::White => Information::HandWhite(hand),
        },
    ))
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
/// Whether `key` is one of the words that name a line of the opening block, and
/// so may be written with either colon ([`colon`]).
fn a_key_that_names_its_line(key: &str) -> bool {
    key == crate::handicap::KIF_KEYWORD || key.ends_with("の持駒")
}

fn information_line_keyvalue(
    shapes: LineShapes,
) -> impl FnMut(&str) -> IResult<&str, Information, VerboseError<&str>> {
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
        let (rest, key) = preceded(padding, is_not("：:\r\n"))(input)?;
        let (rest, mark) = colon(rest)?;
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
        if mark == ':' && !a_key_that_names_its_line(key) {
            return Err(nom::Err::Error(VerboseError::from_error_kind(
                input,
                ErrorKind::OneOf,
            )));
        }
        let (rest, value) = terminated(not_line_ending, line_ending)(rest)?;
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
) -> impl FnMut(&str) -> IResult<&str, InformationData, VerboseError<&str>> {
    move |input| information_lines(shapes, input)
}

fn information_lines(
    shapes: LineShapes,
    input: &str,
) -> IResult<&str, InformationData, VerboseError<&str>> {
    map(
        many0(preceded(
            many0(comment_line),
            alt((
                // Before the key-value rule, which would file the line under a
                // key nobody wrote, and *as one of the alternatives* rather than
                // as a guard: a guard that refuses ends `many0`, and everything
                // below the comment — the rest of the header, the board, the
                // `手合割` — is then read by nobody (R-KIF-004, R-HC-001).
                map(move_comment_line, Information::Comment),
                information_line_preset,
                information_line_hands,
                information_line_keyvalue(shapes),
            )),
        )),
        |v| {
            v.iter().fold(InformationData::default(), |mut acc, info| {
                match info {
                    Information::Preset(p) => acc.preset = Some(*p),
                    Information::HandBlack(h) => acc.hands[0] = *h,
                    Information::HandWhite(h) => acc.hands[1] = *h,
                    Information::KeyValue(k, v) => {
                        acc.map.insert(k.to_owned(), v.to_owned());
                    }
                    Information::Comment(c) => acc.comments.push(c.to_owned()),
                }
                acc
            })
        },
    )(input)
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
        pair(padding, line_ending),
    )(input)
}

fn board(input: &str) -> IResult<&str, [[Piece; 9]; 9], VerboseError<&str>> {
    delimited(
        tuple((
            // The two columns in front of `９` line the file numbers up with the
            // frame below them, so they are padding and not part of the word.
            delimited(
                padding,
                tag("９ ８ ７ ６ ５ ４ ３ ２ １"),
                pair(padding, line_ending),
            ),
            delimited(
                padding,
                tag("+---------------------------+"),
                pair(padding, line_ending),
            ),
        )),
        map(count(board_row, 9), |v| {
            let mut ret = [[Piece::empty(); 9]; 9];
            for (i, row) in v.into_iter().enumerate() {
                for (j, p) in row.into_iter().enumerate() {
                    ret[8 - j][i] = p;
                }
            }
            ret
        }),
        delimited(
            padding,
            tag("+---------------------------+"),
            pair(padding, line_ending),
        ),
    )(input)
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
        // R-NOT says a full-width space usually follows and sometimes does not;
        // nothing says "exactly one", and tsshogi's `moveRegExp` writes `同　*`.
        // A file whose columns were re-aligned by a word processor has two.
        value(None, terminated(tag("同"), take_while(is_padding))),
        map(pair(place_x, place_y), |(x, y)| Some(PlaceFormat { x, y })),
    ))(input)
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

/// Reads the header block, the board and the side-to-move line.
///
/// `shapes` is how the reader says what its own lines look like; see
/// [`LineShapes`]. The block itself is the same in KIF and KI2, and the shapes
/// are the only thing that differs inside it.
///
/// Returns the comments that stood among those lines as well — see
/// [`comments_on_the_starting_position`].
pub(super) fn parse_without_moves(
    shapes: LineShapes,
    input: &str,
) -> IResult<&str, (JsonKifuFormat, Vec<String>), VerboseError<&str>> {
    map(
        tuple((
            informations(shapes),
            opt(board),
            informations(shapes),
            side_to_move_line(shapes),
        )),
        |(info1, opt_board, info2, side_to_move)| {
            let info = InformationData::merged(info1, info2);
            let initial = if let Some(board) = opt_board {
                Some(Initial {
                    preset: Preset::PresetOther,
                    data: Some(StateFormat {
                        color: side_to_move.unwrap_or(Color::Black),
                        board,
                        hands: info.hands,
                    }),
                })
            } else {
                Some(Initial {
                    preset: info.preset.unwrap_or(Preset::PresetHirate),
                    data: None,
                })
            };
            (
                JsonKifuFormat {
                    header: info.map,
                    initial,
                    moves: Vec::new(),
                },
                // R-KIF-010: a comment above the first move belongs to the
                // starting position. The caller owns `moves`, so it puts them
                // there.
                info.comments,
            )
        },
    )(input)
}

/// Puts the comments that stood among the header lines in front of the ones the
/// move reader found above the first move — they are all comments on the
/// starting position (R-KIF-010), and the file's order is the order they were
/// written in.
pub(super) fn comments_on_the_starting_position(
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
mod tests {
    use super::*;
    use crate::normalizer::HIRATE_BOARD;

    // A hand line whose count cannot be read must not fall through to the
    // key-value rule: that files the whole line under `header` and leaves the
    // hand empty — including the pieces written before the broken one. A drop
    // from that hand then fails to normalize, and the message names the move
    // rather than the line that actually broke.
    #[test]
    fn a_hand_line_that_cannot_be_read_is_an_error_not_an_empty_hand() {
        const BOARD: &str = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ ・ ・ ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
";
        for hand in ["歩十九 角二", "角二 歩十九", "歩零", "歩十八 と三"] {
            let kif = format!("{BOARD}先手の持駒：{hand}\n手数----指手---------消費時間--\n");
            let err = crate::parser::parse_kif_str(&kif)
                .err()
                .unwrap_or_else(|| panic!("{hand:?} was accepted"));
            assert!(
                matches!(err, crate::error::ParseError::Kif(_)),
                "{hand:?} gave {err:?}"
            );
        }
    }

    // The counts come from the file, so they can add up past what a `u8` holds.
    // Wrapping would turn a broken hand into a small plausible one and hand it
    // to the rest of the crate as a fact.
    #[test]
    fn a_hand_that_overflows_a_u8_does_not_wrap() {
        // 18 pawns fifteen times: 270, which is 14 once it wraps.
        let overflowing = "歩十八 ".repeat(15);
        assert!(
            information_value_hand(overflowing.trim_end()).is_err(),
            "a hand of 270 must not parse"
        );

        // Stated before *and* after the board. Each line is under the limit on
        // its own, so the guard that matters here is the one merging them.
        let (_, half) =
            information_value_hand("歩十八 歩十八 歩十八 歩十八 歩十八 歩十八 歩十八 歩十八")
                .expect("144 pawns parses");
        assert_eq!(144, half.FU);
        // 288 wraps to 32; saturating keeps it obviously wrong.
        let merged =
            InformationData::merged_hands([half, Hand::default()], [half, Hand::default()]);
        assert_eq!(u8::MAX, merged[0].FU);
    }

    // A line the move list has no shape for is skipped whole — one line, not
    // two. The parser is anchored on the line's first character, and a newline
    // must not be allowed to be it: starting on the newline of a *blank* line
    // takes the line after it as this line's content, so a blank line in the
    // middle of a record destroys the move that follows it (R-KIF-002).
    //
    // `skippable_line` tries `blank_line` first, which hides this. The guard is
    // what keeps the two orderings from meaning different things.
    #[test]
    fn a_skipped_line_never_swallows_the_line_after_it() {
        assert_eq!(
            Ok(("   2 ３四歩(33)\n", "変化：2")),
            not_move_line(NOTHING, "変化：2\n   2 ３四歩(33)\n")
        );
        assert!(not_move_line(NOTHING, "\n   2 ３四歩(33)\n").is_err());
        assert!(not_move_line(NOTHING, "\r\n   2 ３四歩(33)\n").is_err());
        // Padding in front of a blank line does not make it a line with
        // something on it.
        assert!(not_move_line(NOTHING, "　\t \n   2 ３四歩(33)\n").is_err());
    }

    // What a line is does not change with how far in it starts. KIF writes its
    // move lines three columns in (R-KIF-008), and a note or a `まで…` lined up
    // with them is the same line it would be at column 0 — but a reader that
    // asks only at column 0 hands it to the prose skip, and what it said goes
    // with it. The KI2 side of this is
    // `ki2::tests::a_line_is_the_line_it_is_however_far_in_it_starts`.
    #[test]
    fn kif_reads_a_line_however_far_in_it_starts() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        const KIF: &str =
            "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
        const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
        for pad in ["", " ", "   ", "\t", "　", "\u{a0}"] {
            // A comment and a bookmark are kept (R-KIF-010 / R-KIF-011), and
            // prose after them is skipped, whichever column they start in.
            let kif = parse_kif_str(&format!("{KIF}{pad}*コメント\n{pad}&しおり\n{pad}感想戦\n"))
                .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
            assert_eq!(
                Some(&vec![String::from("コメント"), String::from("&しおり")]),
                kif.moves[2].comments.as_ref(),
                "{pad:?}: {:?}",
                kif.moves
            );
            // And the same content reads the same way as `.ki2` (D18).
            let ki2 = parse_ki2_str(&format!("{KI2}{pad}*コメント\n{pad}&しおり\n{pad}感想戦\n"))
                .unwrap_or_else(|e| panic!("{pad:?} as .ki2: {e}"));
            assert_eq!(kif.moves[2].comments, ki2.moves[2].comments, "{pad:?}");
            // A move line is a move line however it is indented.
            let indented = parse_kif_str(&format!(
                "手合割：平手\n手数----指手---------消費時間--\n{pad}   1 ７六歩(77)\n{pad}   2 ８四歩(83)\n"
            ))
            .unwrap_or_else(|e| panic!("{pad:?} on a move line: {e}"));
            assert_eq!(2, indented.moves.len() - 1, "{pad:?}: the moves went");
            // So is a branch header, and the diagnostic under it survives.
            assert!(
                parse_kif_str(&format!("{KIF}\n{pad}変化：2手\n")).is_err(),
                "{pad:?}: `a 変化 block with no moves under it` went quiet"
            );
        }
    }

    #[test]
    fn parse_comment_line() {
        assert!(comment_line("").is_err());
        assert!(comment_line("not comment\n").is_err());
        assert_eq!(
            Ok(("", String::from(" comment"))),
            comment_line("# comment\n")
        );
        // A file need not end with a newline. Insisting on one dropped the last
        // line and said nothing, so the record came back short.
        assert_eq!(
            Ok(("", String::from(" comment"))),
            comment_line("# comment")
        );
    }

    #[test]
    fn parse_not_move_line() {
        assert!(not_move_line(NOTHING, "").is_err());
        assert!(not_move_line(NOTHING, "* comment line\n").is_err());
        assert!(not_move_line(NOTHING, "手数----指手---------消費時間--\n").is_ok());
        assert!(not_move_line(NOTHING, "1 ７六歩(77) ( 0:16/00:00:16)").is_err());
    }

    #[test]
    fn parse_information_preset() {
        assert!(information_line_preset("").is_err());
        assert_eq!(
            Ok(("", Information::Preset(Preset::PresetHirate))),
            information_line_preset("手合割：平手　　\n")
        );
        assert_eq!(
            Ok(("", Information::Preset(Preset::PresetKY))),
            information_line_preset("手合割：香落ち\n")
        );
        assert_eq!(
            Ok(("", Information::Preset(Preset::PresetOther))),
            information_line_preset("手合割：その他\n")
        );
    }

    #[test]
    fn parse_information_hand() {
        assert!(information_line_hands("").is_err());
        assert_eq!(
            Ok((
                "",
                Information::HandBlack(Hand {
                    KE: 1,
                    KI: 1,
                    ..Default::default()
                })
            )),
            information_line_hands("先手の持駒：金　桂　\n")
        );
        assert_eq!(
            Ok((
                "",
                Information::HandWhite(Hand {
                    FU: 15,
                    KY: 2,
                    KE: 3,
                    GI: 2,
                    KI: 3,
                    KA: 1,
                    HI: 0
                })
            )),
            information_line_hands("後手の持駒：角　金三　銀二　桂三　香二　歩十五　\n")
        );
        assert_eq!(
            Ok((
                "",
                Information::HandWhite(Hand {
                    FU: 10,
                    KY: 3,
                    KE: 1,
                    GI: 0,
                    KI: 1,
                    KA: 0,
                    HI: 0
                })
            )),
            information_line_hands("後手の持駒：金　桂　香三　歩十　\n")
        );
        assert_eq!(
            Ok((
                "",
                Information::HandBlack(Hand {
                    KA: 1,
                    ..Default::default()
                })
            )),
            information_line_hands("下手の持駒：角　\n")
        );
    }

    /// A reader that finds no line of its own in any header value — what the
    /// KIF side passes, and what these cases are about.
    const NOTHING: LineShapes = LineShapes {
        carries_a_line: |_| false,
        opens_a_line: opens_a_shared_line,
        opens_a_move_line: opens_a_numbered_line,
    };

    /// One that finds a `▲` enough, for the case that has to be refused.
    const ANY_MOVE_MARK: LineShapes = LineShapes {
        carries_a_line: |value| value.contains('▲'),
        opens_a_line: opens_a_shared_line,
        opens_a_move_line: opens_a_numbered_line,
    };

    // What separates a line that lost its ending from a line that trails off
    // into something the formats do not define. The right-hand column is what
    // `ends_here` does with it: refuse the record, or skip the rest of the line.
    #[test]
    fn what_counts_as_the_line_below() {
        use super::super::{ki2, kif};
        for (shapes, tail) in [
            (kif::SHAPES, "   5 ８八銀(79)   ( 0:01/00:00:05)"),
            // The newline arrived as another byte, so the shapes are looked for
            // past it as well.
            (kif::SHAPES, "\u{0}   5 ８八銀(79)"),
            (kif::SHAPES, ",   5 ８八銀(79)"),
            (kif::SHAPES, "\u{0}*コメント"),
            (kif::SHAPES, ",&しおり"),
            (kif::SHAPES, "\u{0}変化：2手"),
            (kif::SHAPES, "x まで82手で先手の勝ち"),
            (ki2::SHAPES, "\u{0}*コメント"),
            (ki2::SHAPES, "\u{0}変化：2手"),
            (kif::SHAPES, "*コメント"),
            (kif::SHAPES, "&しおり"),
            (kif::SHAPES, "# メモ"),
            (kif::SHAPES, "変化：3手"),
            (kif::SHAPES, "まで82手で先手の勝ち"),
            (ki2::SHAPES, " ▲７六歩 △３四歩"),
            (ki2::SHAPES, "△８四歩"),
            // A branch header this reader cannot parse is still one. Reading
            // only the half-width spelling here let `変化：２手` through as an
            // annotation, and the branch under it carried on as the main line.
            (kif::SHAPES, "変化：２手"),
            (ki2::SHAPES, "変化：２手"),
            (kif::SHAPES, "変化： 2手"),
        ] {
            assert!(
                begins_the_line_below(shapes, tail),
                "{tail:?} is a line of its own"
            );
        }
        for (shapes, tail) in [
            (kif::SHAPES, "+"), // Kifu for Windows marks moves with it
            (kif::SHAPES, "!?"),
            (kif::SHAPES, " 評価値+120"),
            (kif::SHAPES, "( 0:01)"), // a consumed time this reader cannot read
            (kif::SHAPES, "（ 0:01/00:00:01）"),
            (kif::SHAPES, " 55"),
            (kif::SHAPES, "　※好手"),
            (kif::SHAPES, ""),
            // A KIF numbers its move lines, so a `▲` in one is prose.
            (kif::SHAPES, " ※▲２六歩が本筋"),
            (kif::SHAPES, " （▲７六歩まで）"),
            // And what a note opens with is not what a newline turned into.
            (kif::SHAPES, "（まで先手良し）"),
            (kif::SHAPES, "※ 3 分の1"),
            (kif::SHAPES, "[ 3 分]"),
            (kif::SHAPES, "※ 12 手目からの変化"),
            (ki2::SHAPES, "（△８四歩 ▲２六歩）"),
            // And in KI2 a move behind a marker is prose too: what the marker
            // stands in for is a newline, and a note is not one.
            (ki2::SHAPES, " ※△８四歩の変化"),
            (ki2::SHAPES, "（△８四歩）"),
            (ki2::SHAPES, "▲有利"),
            // A newline that is still there was never lost, whatever padding
            // sits before it. Stepping over one reads the line below as this
            // line's overflow and refuses the record over a trailing space.
            (kif::SHAPES, "　\n   2 ３四歩(33)\n"),
            (kif::SHAPES, "\t\n*コメント\n"),
            (kif::SHAPES, " \n変化：2手\n"),
            (ki2::SHAPES, "　\n△８四歩\n"),
            // A note that names the move it is about is prose with a move at
            // the head of it. `まで…` and `変化：` are where D18 puts notes, so
            // a line of KI2 has to be moves the whole way to count as one.
            (ki2::SHAPES, " △８四歩が最善だった"),
            (ki2::SHAPES, "　△３三桂が敗着"),
            (ki2::SHAPES, " △８四歩から"),
            // A branch header that does not own the line it would begin is
            // prose about a branch. `  83 投了    変化：2手  ( 0:00)` is how
            // the annotation column of a move line reads.
            (kif::SHAPES, "    変化：2手     ( 0:00/00:00:00)"),
            (kif::SHAPES, "    変化：２手     ( 0:00/00:00:00)"),
            (kif::SHAPES, " 変化：2手を参照"),
            (ki2::SHAPES, " 変化：２手を参照"),
        ] {
            assert!(
                !begins_the_line_below(shapes, tail),
                "{tail:?} is an annotation, and dropping it loses nothing"
            );
        }
    }

    // What a reader counts as a branch header and what it can consume are the
    // same set, so that no line falls between them: counted, and so not skipped
    // as prose; unreadable, and so reported by the leftover-input check (D1).
    #[test]
    fn every_branch_header_that_counts_as_one_can_be_read_as_one() {
        for (head, ply) in [
            ("変化：2手", 2),
            ("変化：２手", 2),
            ("変化： 2手", 2),
            ("変化：\t１２手", 12),
            ("変化：38", 38),
            // Real records need three digits; these are here because a damaged
            // one must not be fatal. The number saturates rather than failing,
            // because failing would put the line in the difference between the
            // two questions — which is the thing this pair exists to close.
            ("変化：2000手", 2000),
            ("変化：18446744073709551615手", usize::MAX),
            ("変化：18446744073709551616手", usize::MAX),
            ("変化：99999999999999999999手", usize::MAX),
            // Ply 0 is read here. Refusing it belongs to `ki2::branch_header`,
            // the only reader that uses the number (D3).
            ("変化：0手", 0),
            ("変化：０手", 0),
        ] {
            assert!(opens_a_branch_header(head), "{head:?}");
            let (rest, read) =
                branch_header_ply(head).unwrap_or_else(|e| panic!("{head:?}: {e:?}"));
            assert_eq!(ply, read, "{head:?}");
            assert!(rest.is_empty(), "{head:?} left {rest:?}");
        }
        for head in ["変化：ここから", "変化：", "変化：手", "変わり：2手"] {
            assert!(!opens_a_branch_header(head), "{head:?}");
            assert!(branch_header_ply(head).is_err(), "{head:?}");
        }
    }

    // The header block is read twice — before the board and after it — and a
    // comment can stand in either half (R-KIF-010). Both halves and their order
    // are contracts nothing else states: a JKF is a list, and a reader that
    // returns the same comments in a different order returns a different
    // record. Records with a board are the ones that reach this path at all
    // (詰将棋 and any 任意局面, `research/90-gaps.md` GAP-007).
    #[test]
    fn a_comment_on_either_side_of_the_board_is_kept_in_the_order_it_was_written() {
        use crate::parser::parse_kif_str;
        const EMPTY_BOARD: &str = concat!(
            "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
            "+---------------------------+\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
            "+---------------------------+\n",
        );
        // `E` is read by the move reader rather than by the header block, so it
        // is what says the two lists are joined the way the file wrote them.
        let jkf = parse_kif_str(&format!(
            "*A\n後手の持駒：歩\n*B\n{EMPTY_BOARD}*C\n先手の持駒：角\n*D\n先手番\n\
             手数----指手---------消費時間--\n*E\n"
        ))
        .expect("reads");
        assert_eq!(
            Some(&vec![
                String::from("A"),
                String::from("B"),
                String::from("C"),
                String::from("D"),
                String::from("E"),
            ]),
            jkf.moves[0].comments.as_ref(),
            "the halves are joined out of order, or one of them is dropped"
        );
        // And the hands stated on either side of the board are both read.
        let data = jkf.initial.expect("a position").data.expect("a board");
        assert_eq!(1, data.hands[0].KA);
        assert_eq!(1, data.hands[1].FU);
    }

    // A value this crate has no board for still has to land somewhere. The
    // preset rule takes either colon and then declines, so without a landing
    // place the line falls to the prose skip and the record loses even the word
    // the file wrote — GAP-021 keeps it under `header`, which is the lighter
    // half of the same problem. A key nobody knows keeps the full-width colon:
    // otherwise every `key: value` in any text is a header line (D1 / D8).
    #[test]
    fn a_known_key_may_use_either_colon_and_still_lands_somewhere() {
        use crate::parser::parse_kif_str;
        const TAIL: &str = "手数----指手---------消費時間--\n";
        for colon in ['：', ':'] {
            let known = parse_kif_str(&format!("手合割{colon}香落ち\n{TAIL}")).expect("reads");
            assert_eq!(Preset::PresetKY, known.initial.expect("a position").preset);

            for value in ["詰将棋", "香落ち（30分）"] {
                let jkf = parse_kif_str(&format!("手合割{colon}{value}\n{TAIL}"))
                    .unwrap_or_else(|e| panic!("手合割{colon}{value}: {e}"));
                assert_eq!(
                    Some(&String::from(value)),
                    jkf.header.get("手合割"),
                    "手合割{colon}{value}: the line went without a word"
                );
            }
        }
        // A key this reader does not know keeps the full-width colon, so that a
        // file which is not a kifu still says so.
        assert!(parse_kif_str("{\"header\":{},\"moves\":[{}]}\n").is_err());
    }

    // A board diagram reaches the reader through no other path (GAP-007), so a
    // line of one that the readers cannot take must not be taken by the skip
    // instead: the record then comes back as an empty 平手 and the writer saves
    // that over the original (D4). Padding at the end of a line is padding;
    // anything else broken about the diagram is reported (D1).
    #[test]
    fn a_board_is_read_or_reported_but_never_skipped() {
        use crate::parser::parse_kif_str;
        const ROWS: [&str; 12] = [
            "  ９ ８ ７ ６ ５ ４ ３ ２ １",
            "+---------------------------+",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九",
            "+---------------------------+",
        ];
        let record = |rows: &[String]| {
            format!(
                "手合割：詰将棋\n後手の持駒：飛二\n{}先手の持駒：金\n後手番\n",
                rows.concat()
            )
        };
        // Padding at the end of any one line, on any one row.
        for pad in [" ", "　", "\t"] {
            for broken in 0..ROWS.len() {
                let rows: Vec<String> = ROWS
                    .iter()
                    .enumerate()
                    .map(|(i, line)| {
                        if i == broken {
                            format!("{line}{pad}\n")
                        } else {
                            format!("{line}\n")
                        }
                    })
                    .collect();
                let jkf = parse_kif_str(&record(&rows))
                    .unwrap_or_else(|e| panic!("{pad:?} on row {broken}: {e}"));
                assert!(
                    jkf.initial.expect("a position").data.is_some(),
                    "{pad:?} on row {broken}: the board went"
                );
            }
        }
        // And a diagram that is actually broken says so rather than vanishing.
        for (name, broken, line) in [
            ("a short frame", 1, "+--------------------------+\n"),
            ("a half-width rank", 2, "| ・ ・ ・ ・ ・ ・ ・ ・ ・|1\n"),
            ("eight files", 3, "| ・ ・ ・ ・ ・ ・ ・ ・|二\n"),
        ] {
            let rows: Vec<String> = ROWS
                .iter()
                .enumerate()
                .map(|(i, l)| {
                    if i == broken {
                        String::from(line)
                    } else {
                        format!("{l}\n")
                    }
                })
                .collect();
            assert!(parse_kif_str(&record(&rows)).is_err(), "{name}");
        }
        // Including one that stops in the middle of the file.
        let cut: Vec<String> = ROWS[..6].iter().map(|l| format!("{l}\n")).collect();
        assert!(parse_kif_str(&record(&cut)).is_err(), "a diagram cut short");
    }

    // R-KIF-014 asks a *writer* to spell an empty hand `なし`. A reader that
    // refuses the blank one refuses the whole file — board and all — over a
    // line that says nothing, and tsshogi's `readHand` drops an empty section
    // exactly as it drops `なし`.
    #[test]
    fn an_empty_hand_line_is_an_empty_hand() {
        use crate::parser::parse_kif_str;
        const BOARD: &str = concat!(
            "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
            "+---------------------------+\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
            "+---------------------------+\n先手の持駒：なし\n先手番\n",
        );
        for (value, fu, ka) in [
            ("なし", 0, 0),
            ("", 0, 0),
            (" ", 0, 0),
            ("　", 0, 0),
            ("\t", 0, 0),
            ("歩", 1, 0),
            ("歩十八", 18, 0),
            ("歩 角", 1, 1),
            // The padding in front of the value belongs to the value too.
            (" なし", 0, 0),
            (" 歩", 1, 0),
            ("　歩", 1, 0),
            ("\t歩 角", 1, 1),
        ] {
            let jkf = parse_kif_str(&format!("後手の持駒：{value}\n{BOARD}"))
                .unwrap_or_else(|e| panic!("{value:?}: {e}"));
            let hand = jkf
                .initial
                .expect("a position")
                .data
                .expect("a board")
                .hands[1];
            assert_eq!((fu, ka), (hand.FU, hand.KA), "{value:?}");
        }
    }

    // A digit at the head of a line is a KIF move line only when a move follows
    // it. `1図以下、先手優勢` and `35手目まで` are how a KIF annotates itself,
    // and refusing to skip them refuses the record over a note (D17). The same
    // question keeps a KIF move line that ended up in a `.ki2` from being
    // skipped, which would take the move with it (D1).
    #[test]
    fn a_digit_opens_a_move_line_only_when_a_move_follows_it() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        const KIF: &str =
            "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
        const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
        for note in [
            "　1図以下、先手優勢",
            "　3手目は疑問",
            "\t35手目まで",
            " 1図以下",
            "55",
        ] {
            assert!(
                !opens_a_numbered_line(note.trim_start_matches(is_padding)),
                "{note:?}"
            );
            let kif = parse_kif_str(&format!("{KIF}{note}\n"))
                .unwrap_or_else(|e| panic!("{note:?} in a KIF: {e}"));
            assert_eq!(2, kif.moves.len() - 1, "{note:?}");
            let ki2 = parse_ki2_str(&format!("{KI2}{note}\n"))
                .unwrap_or_else(|e| panic!("{note:?} in a KI2: {e}"));
            assert_eq!(2, ki2.moves.len() - 1, "{note:?}");
        }
        // And a numbered line that does carry a move is never skipped, in
        // either format.
        assert!(parse_ki2_str(&format!("{KI2}   3 ２六歩(27)\n")).is_err());
    }

    // A mark at the head of a line is how a move opens, and the indentation in
    // front of it does not change that — `　▲有利でした` is refused exactly as
    // `▲有利でした` is. Before the readers looked past the indentation, the
    // padding itself satisfied the test and the line was skipped instead; the
    // two spellings now agree. Whether a mark *anywhere* in a line should refuse
    // it is GAP-029, still open.
    #[test]
    fn a_mark_opens_a_line_from_wherever_the_line_starts() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        const KIF: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
        for pad in ["", " ", "\t", "　", "\u{a0}"] {
            assert!(
                parse_kif_str(&format!("{KIF}{pad}▲有利でした\n")).is_err(),
                "{pad:?}: a line opening with a mark was skipped"
            );
            assert!(
                parse_ki2_str(&format!("手合割：平手\n▲７六歩\n{pad}△有利でした\n")).is_err(),
                "{pad:?}"
            );
            // A note that only quotes a mark is still prose in KIF (GAP-029).
            assert!(
                parse_kif_str(&format!("{KIF}{pad}※▲２六歩が本筋\n")).is_ok(),
                "{pad:?}"
            );
        }
    }

    // `変化：2手を参照` is prose about a branch. Read as the header of one, the
    // block under it is empty and `a 変化 block with no moves under it` refuses
    // the whole record over a note — the same string D17's table already fixes
    // as an annotation where it lands at the end of a line.
    #[test]
    fn a_branch_header_is_the_whole_of_what_its_number_says() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        const KIF: &str =
            "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n";
        const KI2: &str = "手合割：平手\n▲７六歩 △８四歩\n";
        // A note about a branch, with nothing under it: skipped, in both
        // formats, as it was before this crate started reading the line at all.
        for note in [
            "変化：2手を参照",
            "変化：2手が有力",
            "変化：2手目以下は別途",
            "変化：ここから",
        ] {
            assert!(!a_branch_header_is_all_the_line_says(note), "{note:?}");
            let kif = parse_kif_str(&format!("{KIF}{note}\n"))
                .unwrap_or_else(|e| panic!("{note:?} in a KIF: {e}"));
            assert_eq!(2, kif.moves.len() - 1, "{note:?}");
            let ki2 = parse_ki2_str(&format!("{KI2}{note}\n"))
                .unwrap_or_else(|e| panic!("{note:?} in a KI2: {e}"));
            assert_eq!(2, ki2.moves.len() - 1, "{note:?}");
        }
        // And with a block under it, every one of those is the branch it says
        // it is — the suffix is a note on the header, not a different kind of
        // line. Reading it as prose lets the branch run on as the main line,
        // which changes whose game it is (R-JKF-004).
        for header in [
            "変化：2手",
            "変化：2手 別案",
            "変化：2手（本命）",
            "変化：2",
            "変化：2手目",
            "変化：2手。",
            "変化：2手/A",
        ] {
            assert!(opens_a_branch_header(header), "{header:?}");
            let jkf = parse_kif_str(&format!("{KIF}\n{header}\n   2 ３四歩(33)\n"))
                .unwrap_or_else(|e| panic!("{header:?}: {e}"));
            assert_eq!(
                1,
                jkf.moves.iter().filter(|m| m.forks.is_some()).count(),
                "{header:?}"
            );
            let ki2 = parse_ki2_str(&format!("{KI2}{header}\n△８四歩\n"))
                .unwrap_or_else(|e| panic!("{header:?} in a KI2: {e}"));
            assert_eq!(
                1,
                ki2.moves.iter().filter(|m| m.forks.is_some()).count(),
                "{header:?} in a KI2"
            );
        }
        // A header that says nothing but its number, with nothing under it, is
        // a branch that went missing and still says so (D1).
        assert!(parse_kif_str(&format!("{KIF}\n変化：2手\n")).is_err());
        assert!(parse_ki2_str(&format!("{KI2}変化：2手\n")).is_err());
    }

    // Every line the header block is made of, at every indentation. The skip
    // and the readers have to widen together: a line the readers can no longer
    // take is taken by the skip, and the record comes back `Ok` without it —
    // a tsume whose board is gone reads as an empty 平手, and a handicap that
    // fell into `header` reverses every move's side (R-HC-001 / R-RULE-006).
    //
    // The board is the reason this matters at all: a record with one reaches
    // the reader through no other path (`research/90-gaps.md` GAP-007).
    #[test]
    fn the_header_block_reads_at_any_indentation() {
        use crate::parser::parse_kif_str;
        const BOARD: &str = concat!(
            "  ９ ８ ７ ６ ５ ４ ３ ２ １\n",
            "+---------------------------+\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n",
            "| ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n",
            "+---------------------------+\n",
        );
        for pad in ["", " ", "   ", "\t", "　", "\u{a0}"] {
            let indented = BOARD
                .lines()
                .map(|line| format!("{pad}{line}\n"))
                .collect::<String>();
            let jkf = parse_kif_str(&format!(
                "{indented}{pad}後手の持駒：飛\n{pad}先手の持駒：なし\n{pad}後手番\n"
            ))
            .unwrap_or_else(|e| panic!("{pad:?} on a board: {e}"));
            let data = jkf
                .initial
                .expect("a position")
                .data
                .unwrap_or_else(|| panic!("{pad:?}: the board went"));
            assert_eq!(Color::White, data.color, "{pad:?}: the side to move went");
            assert_eq!(1, data.hands[1].HI, "{pad:?}: the hand went");

            let handicap = parse_kif_str(&format!(
                "{pad}手合割：香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
            ))
            .unwrap_or_else(|e| panic!("{pad:?} on 手合割: {e}"));
            assert_eq!(
                Preset::PresetKY,
                handicap.initial.expect("a position").preset,
                "{pad:?}: the handicap went, and every side with it"
            );

            let header = parse_kif_str(&format!(
                "{pad}棋戦：竜王戦\n手数----指手---------消費時間--\n   1 ７六歩(77)\n"
            ))
            .unwrap_or_else(|e| panic!("{pad:?} on a header: {e}"));
            assert_eq!(
                Some(&String::from("竜王戦")),
                header.header.get("棋戦"),
                "{pad:?}: the indentation went into the key: {:?}",
                header.header
            );
        }
    }

    // R-KIF-014 / D5: tsshogi matches `[：:]` where a keyword names the line, so
    // a record it opens has to open here. Not on the generic key-value line,
    // which takes anything as a key — see `colon`.
    #[test]
    fn a_keyword_names_its_line_with_either_colon() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        for colon in ['：', ':'] {
            let kif = parse_kif_str(&format!(
                "手合割{colon}香落ち\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
            ))
            .unwrap_or_else(|e| panic!("手合割{colon}: {e}"));
            assert_eq!(Preset::PresetKY, kif.initial.expect("a position").preset);
            assert_eq!(Color::White, kif.moves[1].move_.expect("a move").color);

            let ki2 = parse_ki2_str(&format!(
                "手合割{colon}平手\n▲７六歩 △８四歩 ▲２六歩\n変化{colon}3手\n▲２五歩\n"
            ))
            .unwrap_or_else(|e| panic!("変化{colon}: {e}"));
            assert!(
                ki2.moves[3].forks.is_some(),
                "変化{colon}: the branch read as the main line carrying on: {:?}",
                ki2.moves
            );
        }
        // And a file that is not a kifu stays one. A half-width colon on the
        // generic key-value line would make a header out of every `key: value`.
        assert!(parse_kif_str("{\"header\":{},\"moves\":[{}]}\n").is_err());
    }

    // The padding after a value belongs to the value. R-KIF-014 is the shape
    // the consumer's TS side also reads (tsshogi splits the hand on spaces and
    // drops the empty pieces), so a record it opens has to open here too — and
    // a `手合割` left to the key-value rule is a handicap the board never sees,
    // which reads back as 平手 with every side reversed (R-HC-001 / R-RULE-006).
    #[test]
    fn padding_after_a_header_value_belongs_to_the_value() {
        use crate::parser::parse_kif_str;
        const BOARD: &str = "  ９ ８ ７ ６ ５ ４ ３ ２ １\n\
            +---------------------------+\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|一\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|二\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|三\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|七\n| ・ ・ ・ ・ ・ ・ ・ ・ ・|八\n\
            | ・ ・ ・ ・ ・ ・ ・ ・ ・|九\n+---------------------------+\n\
            先手の持駒：なし\n先手番\n手数----指手---------消費時間--\n";
        for pad in ["", " ", "　", "\t", "\u{a0}"] {
            for hand in ["なし", "歩", "歩二 角"] {
                let jkf = parse_kif_str(&format!("後手の持駒：{hand}{pad}\n{BOARD}"))
                    .unwrap_or_else(|e| panic!("{hand:?} + {pad:?}: {e}"));
                assert!(jkf.initial.expect("a position").data.is_some());
            }
            let jkf = parse_kif_str(&format!(
                "手合割：香落ち{pad}\n手数----指手---------消費時間--\n   1 ３四歩(33)\n"
            ))
            .unwrap_or_else(|e| panic!("手合割 + {pad:?}: {e}"));
            assert_eq!(Preset::PresetKY, jkf.initial.expect("a position").preset);
            assert_eq!(
                Color::White,
                jkf.moves[1].move_.expect("a move").color,
                "{pad:?}: the handicap went, and the sides with it"
            );
        }
    }

    #[test]
    fn parse_information_keyvalue() {
        assert!(information_line_keyvalue(NOTHING)("").is_err());
        assert!(information_line_keyvalue(NOTHING)("# comment\n").is_err());
        assert!(information_line_keyvalue(NOTHING)("key：value with not line ending").is_err());
        assert_eq!(
            Ok((
                "",
                Information::KeyValue(String::from("key"), String::from("value"))
            )),
            information_line_keyvalue(NOTHING)("key：value\n")
        );
        // And a value the reader does find one in: the line under it was read as
        // part of it, so the record is short by however much that line held.
        assert!(information_line_keyvalue(ANY_MOVE_MARK)("手合割：平手 ▲７六歩\n").is_err());
        // R-KIF-010 / R-KIF-011: a comment and a bookmark hold free text, and
        // free text holds `：`. Filed as a header they are gone from the record
        // and a key nobody wrote is in it.
        assert!(information_line_keyvalue(NOTHING)("*（主催：新聞三社連合）\n").is_err());
        assert!(information_line_keyvalue(NOTHING)("&しおり：ここ\n").is_err());
    }

    // R-KIF-010: comments before the first move belong to the starting position.
    // KIF ends its header block with `手数----指手---------消費時間--`, which
    // R-KIF-012 says a record need not have and KI2 has no equivalent of — so a
    // header rule that eats `*` lines eats them out of every KI2 and out of any
    // KIF written without that line.
    #[test]
    fn a_comment_over_the_first_move_is_not_a_header() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        let ki2 = parse_ki2_str("手合割：平手\n*（主催：新聞三社連合）\n▲７六歩 △３四歩\n")
            .expect("reads");
        assert_eq!(
            Some(&vec![String::from("（主催：新聞三社連合）")]),
            ki2.moves[0].comments.as_ref(),
            "header: {:?}",
            ki2.header
        );
        assert!(ki2.header.is_empty(), "{:?}", ki2.header);
        let kif = parse_kif_str("手合割：平手\n*（主催：新聞三社連合）\n   1 ７六歩(77)\n")
            .expect("reads");
        assert_eq!(ki2.moves[0].comments, kif.moves[0].comments);
        assert!(kif.header.is_empty(), "{:?}", kif.header);
    }

    // And it does not end the header block either. A rule that refuses the line
    // instead of reading it stops `many0`, and everything under the comment —
    // the rest of the header, the board, the `手合割` — is read by nobody. A
    // handicap lost that way defaults to 平手, and 平手 has Black moving first
    // where the record has White (R-HC-001 / R-RULE-006): every side flips.
    #[test]
    fn a_comment_among_the_header_lines_does_not_end_the_header() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        for opener in ["*メモ：あ", "&しおり：あ", "# Kifu for Windows"] {
            let kif = parse_kif_str(&format!(
                "先手：山田\n{opener}\n後手：田中\n手合割：香落ち\n\
                 手数----指手---------消費時間--\n   1 ３四歩(33)\n"
            ))
            .unwrap_or_else(|e| panic!("{opener}: {e}"));
            assert_eq!(
                Some(&String::from("田中")),
                kif.header.get("後手"),
                "{opener}: the header under the comment is gone: {:?}",
                kif.header
            );
            let initial = kif.initial.as_ref().expect("a starting position");
            assert_eq!(Preset::PresetKY, initial.preset, "{opener}");
            assert_eq!(
                Color::White,
                kif.moves[1].move_.as_ref().expect("a move").color,
                "{opener}: the handicap went, and the sides with it"
            );
        }
        // KI2 has no `手数----` line to end the block with (R-KI2-002), so this
        // is the only thing standing between a note in its header and the rest
        // of the record.
        let ki2 = parse_ki2_str("開始日時：2021\n*（主催：…）\n先手：藤井\n後手：豊島\n▲７六歩\n")
            .expect("reads");
        assert_eq!(3, ki2.header.len(), "{:?}", ki2.header);
    }

    #[test]
    fn parse_informations() {
        assert_eq!(
            Ok(("", InformationData::default())),
            informations(NOTHING)("")
        );
        assert_eq!(
            Ok((
                "",
                InformationData {
                    map: [(String::from("key"), String::from("value"))]
                        .into_iter()
                        .collect(),
                    ..Default::default()
                }
            )),
            informations(NOTHING)("# comment\n# comment：comment\nkey：value\n")
        );
    }

    // R-NOT-006: a writer uses the standard form only, and both writers are
    // writing the same notation — so one table, and every entry in it has to be
    // one the reader takes back. The reader takes the variants too (R-KI2-005),
    // so a table that drifted still produces readable files; only a round trip
    // that checks *which* piece comes back says the two writers disagree.
    #[test]
    fn every_move_word_reads_back_as_the_piece_it_names() {
        for kind in [
            Kind::FU,
            Kind::KY,
            Kind::KE,
            Kind::GI,
            Kind::KI,
            Kind::KA,
            Kind::HI,
            Kind::OU,
            Kind::TO,
            Kind::NY,
            Kind::NK,
            Kind::NG,
            Kind::UM,
            Kind::RY,
        ] {
            let word = crate::notation::move_word(kind);
            let (rest, read) =
                piece_kind(word).unwrap_or_else(|e| panic!("{kind:?} spelled {word:?}: {e:?}"));
            assert_eq!(kind, read, "{word:?}");
            assert!(rest.is_empty(), "{word:?} left {rest:?}");
        }
    }

    #[test]
    fn parse_piece_kind() {
        assert!(piece_kind("").is_err());
        assert_eq!(Ok(("", Kind::FU)), piece_kind("歩"));
        assert_eq!(Ok(("", Kind::OU)), piece_kind("玉"));
        assert_eq!(Ok(("", Kind::OU)), piece_kind("王"));
        assert_eq!(Ok(("", Kind::RY)), piece_kind("龍"));
        assert_eq!(Ok(("", Kind::RY)), piece_kind("竜"));
        assert_eq!(Ok(("", Kind::NY)), piece_kind("成香"));
        assert_eq!(Ok(("", Kind::NK)), piece_kind("成桂"));
        assert_eq!(Ok(("", Kind::NG)), piece_kind("成銀"));
        assert_eq!(Ok(("", Kind::NY)), piece_kind("杏"));
        assert_eq!(Ok(("", Kind::NK)), piece_kind("圭"));
        assert_eq!(Ok(("", Kind::NG)), piece_kind("全"));
    }

    #[test]
    fn parse_board_piece() {
        assert!(board_piece("").is_err());
        assert_eq!(Ok(("", Piece::empty())), board_piece(" ・"));
        assert_eq!(
            Ok((
                "",
                Piece {
                    color: Some(Color::Black),
                    kind: Some(Kind::FU)
                }
            )),
            board_piece(" 歩")
        );
        assert_eq!(
            Ok((
                "",
                Piece {
                    color: Some(Color::White),
                    kind: Some(Kind::FU)
                }
            )),
            board_piece("v歩")
        );
    }

    #[test]
    fn parse_board_row() {
        let rows = (0..9)
            .map(|i| (0..9).map(|j| HIRATE_BOARD[8 - j][i]).collect::<Vec<_>>())
            .collect::<Vec<_>>();
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[0].clone())), board_row("|v香v桂v銀v金v玉v金v銀v桂v香|一\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[1].clone())), board_row("| ・v飛 ・ ・ ・ ・ ・v角 ・|二\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[2].clone())), board_row("|v歩v歩v歩v歩v歩v歩v歩v歩v歩|三\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[3].clone())), board_row("| ・ ・ ・ ・ ・ ・ ・ ・ ・|四\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[4].clone())), board_row("| ・ ・ ・ ・ ・ ・ ・ ・ ・|五\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[5].clone())), board_row("| ・ ・ ・ ・ ・ ・ ・ ・ ・|六\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[6].clone())), board_row("| 歩 歩 歩 歩 歩 歩 歩 歩 歩|七\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[7].clone())), board_row("| ・ 角 ・ ・ ・ ・ ・ 飛 ・|八\n"));
        #[rustfmt::skip]
        assert_eq!(Ok(("", rows[8].clone())), board_row("| 香 桂 銀 金 玉 金 銀 桂 香|九\n"));
    }

    #[test]
    fn parse_board() {
        assert_eq!(
            Ok(("", HIRATE_BOARD)),
            board(
                &r#"
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
|v香v桂v銀v金v玉v金v銀v桂v香|一
| ・v飛 ・ ・ ・ ・ ・v角 ・|二
|v歩v歩v歩v歩v歩v歩v歩v歩v歩|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| 歩 歩 歩 歩 歩 歩 歩 歩 歩|七
| ・ 角 ・ ・ ・ ・ ・ 飛 ・|八
| 香 桂 銀 金 玉 金 銀 桂 香|九
+---------------------------+
"#[1..]
            )
        );
    }
}
