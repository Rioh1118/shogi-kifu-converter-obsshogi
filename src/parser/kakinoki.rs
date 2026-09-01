use crate::jkf::*;
use nom::branch::alt;
use nom::bytes::complete::{is_not, tag};
use nom::character::complete::{line_ending, none_of, not_line_ending, one_of, space0};
use nom::combinator::{eof, map, map_res, opt, value};
use nom::error::{ErrorKind, ParseError, VerboseError};
use nom::multi::{count, many0, many1};
use nom::sequence::{delimited, pair, preceded, separated_pair, terminated, tuple};
use nom::IResult;
use std::collections::HashMap;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Information {
    Preset(Preset),
    HandBlack(Hand),
    HandWhite(Hand),
    KeyValue(String, String),
}

#[derive(Debug, Default, PartialEq, Eq)]
struct InformationData {
    preset: Option<Preset>,
    hands: [Hand; 2],
    map: HashMap<String, String>,
}

impl InformationData {
    fn merged(lhs: Self, rhs: Self) -> InformationData {
        InformationData {
            preset: lhs.preset.or(rhs.preset),
            hands: Self::merged_hands(lhs.hands, rhs.hands),
            map: lhs.map.into_iter().chain(rhs.map).collect(),
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
/// One table. The KI2 reader spells its moves with them, and both readers have
/// to agree about which characters those are: `not_move_line` below and
/// `ki2::a_line_only_prose_opens` each decide whether to skip a line by looking
/// for one, and a mark only one of them knows makes a line that is skipped by
/// one reader and kept by the other.
///
/// The variants R-NOT-001 also lists (`☗`/`☖`, `⛊`/`⛉`, `▼`/`▽`) are not read yet:
/// `research/90-gaps.md` GAP-024, which names the three places that have to
/// learn a new one together.
pub(super) const SIDE_MARKS: [(char, Color); 2] = [('▲', Color::Black), ('△', Color::White)];

/// The spaces a line can be padded with. Full-width among them: KI2 is a record
/// people read (R-KI2-001), and what people paste is padded either way.
const SPACES: [char; 3] = [' ', '\t', '　'];

/// The shapes of the format being read.
///
/// The header block, the board and the side-to-move line are the same in KIF
/// and KI2, but what a line of one of them can have swallowed is not: KIF puts
/// its moves on numbered lines, KI2 on `▲`/`△` runs. A reader that looks for
/// both finds the other format's shape in this one's prose — `※▲２六歩が本筋`
/// after a KIF move, which is a note and not a line — and refuses records with
/// nothing wrong with them.
///
/// So each reader says what its own lines look like, and this module holds only
/// what they share.
#[derive(Clone, Copy)]
pub(super) struct LineShapes {
    /// Whether a header value carries a line of this format. A header value is
    /// free text (R-KIF-004), so this is about what the text carries rather than
    /// where it stops — see [`information_line_keyvalue`].
    pub(super) carries_a_line: fn(&str) -> bool,
    /// Whether text that starts where a line starts opens one.
    pub(super) opens_a_line: fn(&str) -> bool,
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
/// A number this reader cannot use is still a number. The parsers want a
/// half-width digit right after the colon, but a line spelled `変化：２手` says a
/// branch starts just as plainly, and this question is asked where the answer
/// decides whether the line is *kept* — by the line-end rule (D17) and by the
/// KI2 skip. Reading the narrower shape here is what turns a branch the reader
/// cannot parse into a branch it never saw: the header is skipped and its moves
/// carry on as the main line (R-JKF-004). Whether the number can be used is the
/// parsers' question, and they ask it themselves.
pub(super) fn opens_a_branch_header(head: &str) -> bool {
    head.strip_prefix("変化：")
        .map(|rest| rest.trim_start_matches(SPACES))
        .is_some_and(|rest| {
            rest.starts_with(|c: char| c.is_ascii_digit() || ('０'..='９').contains(&c))
        })
}

/// Whether `head` is the beginning of a `<手数> <指し手>` line
/// (R-KIF-005 / R-KIF-008).
///
/// The number on its own is not the shape. A `( 0:01)` this reader has no shape
/// for and a bare `55` both carry digits and neither is a line — what makes one
/// is a number, then space, then something for the number to be about.
pub(super) fn opens_a_numbered_line(head: &str) -> bool {
    let after_digits = head.trim_start_matches(|c: char| c.is_ascii_digit());
    after_digits.len() < head.len()
        && after_digits.starts_with(SPACES)
        && !after_digits.trim_start_matches(SPACES).is_empty()
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
fn begins_the_line_below(shapes: LineShapes, tail: &str) -> bool {
    let head = tail.trim_start_matches(SPACES);
    if (shapes.opens_a_line)(head) {
        return true;
    }
    match head.chars().next() {
        Some(c) if !NOTE_MARKERS.contains(&c) => {
            (shapes.opens_a_line)(head[c.len_utf8()..].trim_start_matches(SPACES))
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
    if let Ok(ended) = preceded(space0, end_of_line)(rest) {
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

/// A line with nothing on it but spaces.
///
/// KIF puts one before each `変化：` block, and R-KIF-002 lets one sit anywhere
/// in the move list.
pub(super) fn blank_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    terminated(space0, line_ending)(input)
}

/// A `#` line: a note from the program that wrote the file (R-KIF-002).
pub(super) fn program_comment_line(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    comment_line(input)
}

fn comment_line(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    map(
        delimited(tag("#"), not_line_ending, end_of_line),
        String::from,
    )(input)
}

/// A line that is none of the shapes the move list is made of, skipped whole.
///
/// What it declines to start on is not every shape the reader knows — it is the
/// ones that mean something else at the head of a line: a space or a line ending
/// (a blank line belongs to the caller), a digit (a KIF move line), `*` and `&`
/// (a comment and a bookmark, R-KIF-010 / R-KIF-011), and [`SIDE_MARKS`] (a KI2
/// move). `#`, `変化：` and `まで…` are **not** among them, so a caller that
/// wants one of those read as itself has to try it before this
/// (`kif::skippable_line`).
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
pub(super) fn not_move_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    // Spelled out rather than built from [`SIDE_MARKS`] because `none_of` takes
    // a pattern, not a table. GAP-024 names this as one of the three places a
    // new mark has to be added to.
    delimited(none_of(" \r\n0123456789*&▲△"), not_line_ending, end_of_line)(input)
}

pub(super) fn move_comment_line(input: &str) -> IResult<&str, String, VerboseError<&str>> {
    alt((
        map(
            delimited(tag("*"), not_line_ending, end_of_line),
            String::from,
        ),
        map(delimited(tag("&"), not_line_ending, end_of_line), |s| {
            String::from("&") + s
        }),
    ))(input)
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
    alt((
        value(Hand::default(), tag("なし")),
        map_res(
            many1(terminated(
                pair(piece_kind, map(opt(kansuji), |o| o.unwrap_or(1))),
                many0(one_of(" 　")),
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
    let (rest, _) = many0(one_of(" 　"))(rest)?;
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
            pair(tag(crate::handicap::KIF_KEYWORD), tag("：")),
            information_value_preset,
        ),
        line_ending,
    )(input)
}

fn information_line_hands(input: &str) -> IResult<&str, Information, VerboseError<&str>> {
    let line = input;
    let (rest, color) = terminated(
        alt((
            value(Color::Black, tag("先手")),
            value(Color::White, tag("後手")),
            value(Color::Black, tag("下手")),
            value(Color::White, tag("上手")),
        )),
        tag("の持駒："),
    )(input)?;
    // Past the prefix this line states a hand, whatever follows. Reporting a
    // recoverable error would send it to `information_line_keyvalue`, which
    // files the whole line under `header` and leaves the hand empty — including
    // the pieces written *before* the one that could not be read. A later drop
    // from that hand then fails to normalize, and the message names the move
    // rather than the line that actually broke.
    let fail = |_| nom::Err::Failure(VerboseError::from_error_kind(line, ErrorKind::Tag));
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
fn information_line_keyvalue(
    shapes: LineShapes,
) -> impl FnMut(&str) -> IResult<&str, Information, VerboseError<&str>> {
    move |input| {
        let (rest, (key, value)) = terminated(
            separated_pair(is_not("：\r\n"), tag("："), not_line_ending),
            line_ending,
        )(input)?;
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
            tag("|"),
            count(board_piece, 9),
            preceded(tag("|"), one_of("一二三四五六七八九")),
        ),
        line_ending,
    )(input)
}

fn board(input: &str) -> IResult<&str, [[Piece; 9]; 9], VerboseError<&str>> {
    delimited(
        tuple((
            terminated(tag("  ９ ８ ７ ６ ５ ４ ３ ２ １"), line_ending),
            terminated(tag("+---------------------------+"), line_ending),
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
        terminated(tag("+---------------------------+"), line_ending),
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
        value(None, terminated(tag("同"), opt(tag("　")))),
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
            let (rest, color) = alt((
                value(Color::Black, tag("先手番")),
                value(Color::White, tag("後手番")),
                value(Color::Black, tag("下手番")),
                value(Color::White, tag("上手番")),
            ))(input)?;
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
pub(super) fn parse_without_moves(
    shapes: LineShapes,
    input: &str,
) -> IResult<&str, JsonKifuFormat, VerboseError<&str>> {
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
            JsonKifuFormat {
                header: info.map,
                initial,
                moves: Vec::new(),
            }
        },
    )(input)
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
            Ok(("   2 ３四歩(33)\n", "化：2")),
            not_move_line("変化：2\n   2 ３四歩(33)\n")
        );
        assert!(not_move_line("\n   2 ３四歩(33)\n").is_err());
        assert!(not_move_line("\r\n   2 ３四歩(33)\n").is_err());
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
        assert!(not_move_line("").is_err());
        assert!(not_move_line("* comment line\n").is_err());
        assert!(not_move_line("手数----指手---------消費時間--\n").is_ok());
        assert!(not_move_line("1 ７六歩(77) ( 0:16/00:00:16)").is_err());
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
    };

    /// One that finds a `▲` enough, for the case that has to be refused.
    const ANY_MOVE_MARK: LineShapes = LineShapes {
        carries_a_line: |value| value.contains('▲'),
        opens_a_line: opens_a_shared_line,
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
        ] {
            assert!(
                !begins_the_line_below(shapes, tail),
                "{tail:?} is an annotation, and dropping it loses nothing"
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
