use super::kakinoki::{
    blank_line, broken_line, ends_here, move_comment_line, move_to, not_move_line,
    opens_a_branch_header, opens_a_shared_line, parse_without_moves, piece_kind, LineShapes,
    SPACES,
};
use crate::jkf::*;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{digit1, space0};
use nom::combinator::{map, map_res, opt, value};
use nom::error::{ParseError, VerboseError};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, separated_pair, terminated, tuple};
use nom::IResult;

fn move_from(input: &str) -> IResult<&str, Option<PlaceFormat>, VerboseError<&str>> {
    alt((
        // A drop has no origin, and JKF says so by leaving `from` out
        // (R-JKF-003). KIF marks it with 打 on every drop (R-KIF-006).
        value(None, tag("打")),
        move |input| {
            let (rest, d): (&str, u8) =
                delimited(tag("("), map_res(digit1, str::parse), tag(")"))(input)?;
            let (x, y) = (d / 10, d % 10);
            // R-KIF-005: an origin is `(11)` through `(99)`. `(00)` in
            // particular is CSA's spelling for a drop and this crate's marker
            // for an origin the notation does not state — reading it as either
            // would turn "a square we could not read" into a different move.
            if !(1..=9).contains(&x) || !(1..=9).contains(&y) {
                return Err(nom::Err::Failure(VerboseError::from_error_kind(
                    input,
                    nom::error::ErrorKind::Verify,
                )));
            }
            Ok((rest, Some(PlaceFormat { x, y })))
        },
    ))(input)
}

/// The KIF outcome words, longest first so that a word is never cut short by a
/// prefix of itself. [`MoveSpecial::from_kif_word`] holds the mapping.
const KIF_SPECIAL_WORDS: [&str; 12] = [
    "切れ負け",
    "入玉勝ち",
    "反則負け",
    "反則勝ち",
    "不戦勝",
    "不戦敗",
    "千日手",
    "持将棋",
    "投了",
    "中断",
    "詰み",
    "不詰",
];

/// Parses an outcome word. `side_to_move` decides the direction of 反則勝ち.
fn move_special(
    side_to_move: Color,
) -> impl FnMut(&str) -> IResult<&str, MoveFormat, VerboseError<&str>> {
    move |input| {
        for word in KIF_SPECIAL_WORDS {
            if let Ok((rest, _)) = tag::<_, _, VerboseError<&str>>(word)(input) {
                if let Some(special) = MoveSpecial::from_kif_word(word, side_to_move) {
                    return Ok((
                        rest,
                        MoveFormat {
                            special: Some(special),
                            ..Default::default()
                        },
                    ));
                }
            }
        }
        Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Alt,
        )))
    }
}

/// What a KIF line looks like.
///
/// `carries_a_line` is always false. Not because a move line cannot be joined to
/// a header, but because nothing tells the two apart: a KIF move line opens with
/// a number and a space, and so does `棋戦：第 3 回`. Refusing the one to catch
/// the other rejects records nothing is wrong with (`research/90-gaps.md`
/// GAP-020).
///
/// `▲`/`△` is not among the shapes either. KIF numbers its move lines, so a
/// `▲` in a KIF is prose — `※▲２六歩が本筋` after a move, `（▲７六歩まで）` — and
/// reading it as a line refuses records that are whole.
pub(super) const SHAPES: LineShapes = LineShapes {
    carries_a_line: |_| false,
    opens_a_line: opens_a_kif_line,
};

/// Whether `head` is the beginning of a `<手数> <指し手>` line
/// (R-KIF-005 / R-KIF-008).
///
/// The number on its own is not the shape. A `( 0:01)` this reader has no shape
/// for and a bare `55` both carry digits and neither is a line — what makes one
/// is a number, then space, then something for the number to be about.
fn opens_a_numbered_line(head: &str) -> bool {
    let after_digits = head.trim_start_matches(|c: char| c.is_ascii_digit());
    after_digits.len() < head.len()
        && after_digits.starts_with(SPACES)
        && !after_digits.trim_start_matches(SPACES).is_empty()
}

fn opens_a_kif_line(head: &str) -> bool {
    opens_a_shared_line(head) || opens_a_numbered_line(head)
}

/// The `変化：<N>手` line that opens a branch.
///
/// The number is read only to be thrown away: the tree comes from the ply
/// numbers on the move lines, not from this declaration (D3). The line is still
/// matched rather than left to [`not_move_line`], because a line the reader has
/// no shape for is skipped whole — so a `変化：` joined with the move under it
/// takes a whole branch with it, and the record comes back `Ok` with one fewer.
/// Returns the line itself, which is where an error about the branch has to
/// point.
fn branch_header_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    // `手` is what the writers put after the number, but nothing requires it of
    // a reader — tsshogi's `branchRegExp` reads the number and stops.
    let (rest, _) = tuple((tag("変化："), digit1, opt(tag("手"))))(input)?;
    let (rest, _) = ends_here(SHAPES, input, rest)?;
    Ok((rest, input))
}

/// A line between the runs of moves: blank, a `変化：<N>手` header, or one the
/// move list has no shape for — `まで<N>手で<結末>`, the `手数----指手---` rule,
/// a closing remark.
///
/// Only used before the main line, where a `変化：` is nothing to act on: a
/// branch cannot come before the moves it is an alternative to, and one written
/// there anyway is what tsshogi skips as well.
fn skippable_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    alt((blank_line, branch_header_line, not_move_line))(input)
}

/// The same, except that a `変化：` line is left where it is.
///
/// [`entire_moves`] is the one that reads branch headers, because it is the one
/// that knows whether a run follows. A line that merely looks like one is
/// declined too: a header the reader refuses ([`branch_header_line`]) has to
/// reach that loop to be refused, not be skipped as unreadable prose.
fn skippable_line_except_a_branch_header(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    if opens_a_branch_header(input) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Not,
        )));
    }
    alt((blank_line, not_move_line))(input)
}

/// Skips the blank lines and `#` lines that may sit between two moves of a run
/// (R-KIF-002). Ending the run on one leaves every move after it to be read as
/// a branch of a ply that does not exist, and dropped (GAP-008).
///
/// Only these two. A line the format has no meaning for still ends the run, and
/// the leftover-input check then reports it (D1) rather than guessing.
///
/// Written by hand rather than as `many0(alt((blank_line, program_comment_line)))`
/// because it runs once per move and almost always matches nothing: the nom
/// version allocates a `VerboseError` on each of those misses, which cost 13%
/// of the time to read the whole corpus.
fn skip_interruptions(mut input: &str) -> &str {
    loop {
        let rest = if let Some(rest) = input.strip_prefix('#') {
            rest
        } else {
            let trimmed = input.trim_start_matches([' ', '\t']);
            if !(trimmed.starts_with('\n') || trimmed.starts_with("\r\n")) {
                return input;
            }
            trimmed
        };
        input = match rest.find('\n') {
            Some(i) => &rest[i + 1..],
            None => return "",
        };
    }
}

fn move_move(input: &str) -> IResult<&str, MoveFormat, VerboseError<&str>> {
    map(
        tuple((move_to, piece_kind, opt(tag("成")), move_from)),
        |(to, kind, promote, from)| {
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::Black, // To be replaced
                    from,
                    to: to.unwrap_or_default(), // Might be (0, 0) if it's the same place as previous
                    piece: kind,
                    same: if to.is_none() { Some(true) } else { None },
                    // R-KIF-006: KIF never writes 不成, so on a move that has an
                    // origin the absence of `成` is the record saying the move
                    // did not promote — not the record saying nothing. Leaving
                    // it empty makes it look like the latter, and the normalizer
                    // then reads the piece name to decide (R-CSA-007, which is
                    // how CSA states a promotion) and can put a `成` into a
                    // record that has none.
                    //
                    // A drop has nothing to say either way: a piece enters the
                    // board unpromoted, and R-NOT-005 has no 成/不成 for it.
                    promote: from.map(|_| promote.is_some()),
                    capture: None,
                    relative: None,
                }),
                ..Default::default()
            }
        },
    )(input)
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

fn move_time(input: &str) -> IResult<&str, Time, VerboseError<&str>> {
    delimited(
        tag("("),
        map(
            separated_pair(
                delimited(space0, move_time_format, space0),
                tag("/"),
                delimited(space0, move_time_format, space0),
            ),
            |(now, total)| Time { now, total },
        ),
        tag(")"),
    )(input)
}

// The move parsers below take `(start, input)` and are applied directly rather
// than handing back a parser value, because each needs `start` — whose turn ply
// 1 is — and none of them is used inside a combinator. `move_special` is the
// exception: `alt` needs a parser value, so it returns one.
//
/// Reads one `<ply> <move>` line.
///
/// `known_side` is whose turn this line is when the run has already been read
/// far enough to know. KIF numbers an outcome line as a ply of its own, so a
/// record that was interrupted and resumed has more plies than moves and the
/// parity of the number stops matching the turn — which matters because
/// 反則勝ち names its winner only through whose turn it is (R-KIF-007).
/// `start` is the fallback for the first line of a run.
fn move_line(
    start: Color,
    known_side: Option<Color>,
    input: &str,
) -> IResult<&str, (usize, MoveFormat), VerboseError<&str>> {
    let line = input;
    // The ply number has to be read before the rest: it decides whose turn it
    // is, and 反則勝ち means the *other* player committed the foul.
    let (input, i) = preceded(space0, map_res(digit1, str::parse::<usize>))(input)?;
    let side_to_move = known_side.unwrap_or_else(|| crate::handicap::side_to_move_at_ply(start, i));
    let (input, mut mf) = preceded(space0, alt((move_special(side_to_move), move_move)))(input)?;
    let (input, time) = preceded(space0, opt(move_time))(input)?;
    // R-KIF-005 / R-KIF-008 say what a move line is made of — the ply, the move,
    // and the time that may or may not follow it — and say nothing about what
    // may come after. So what may come after is whatever is not a line: reading
    // to the end and throwing it away takes the line underneath with it when the
    // newline between them is lost, and a move goes missing from a record that
    // still comes back `Ok`. `ends_here` draws that line.
    let (input, _) = ends_here(SHAPES, line, input)?;
    if let Some(mmf) = &mut mf.move_ {
        mmf.color = side_to_move;
    }
    mf.time = time;
    Ok((input, (i, mf)))
}

fn move_with_comments(
    start: Color,
    known_side: Option<Color>,
    input: &str,
) -> IResult<&str, (usize, MoveFormat), VerboseError<&str>> {
    let (input, (i, mf)) = move_line(start, known_side, input)?;
    let (input, comments) = many0(move_comment_line)(input)?;
    Ok((
        input,
        (
            i,
            MoveFormat {
                comments: Some(comments).filter(|v| !v.is_empty()),
                ..mf
            },
        ),
    ))
}

/// Whose turn follows `mf`, which `side` was the turn of.
fn next_side(mf: &MoveFormat, side: Color) -> Color {
    if mf.move_.is_none() {
        return side;
    }
    match side {
        Color::Black => Color::White,
        Color::White => Color::Black,
    }
}

fn moves_with_index(
    start: Color,
    input: &str,
) -> IResult<&str, (usize, Vec<MoveFormat>), VerboseError<&str>> {
    // A run can open with comment lines, and the node they make carries no ply:
    // JKF lets a node hold only comments (R-JKF-002) and KIF has no numbered
    // line for one, so `to_kif` writes them straight after the `変化：N手`
    // header. Refusing them here means this crate writes `変化：` blocks its own
    // reader rejects — and a `&` bookmark in the same place was worse, swallowed
    // as an unreadable line and gone without a word (R-KIF-011).
    let (input, leading) = opt(many1(move_comment_line))(input)?;
    let (mut input, (first_ply, first)) = move_with_comments(start, None, input)?;
    // Whose turn the next line is. Only a move passes the turn; an outcome takes
    // a ply number without taking a turn, which is why the parity of the number
    // cannot be trusted past one.
    let mut side = next_side(
        &first,
        crate::handicap::side_to_move_at_ply(start, first_ply),
    );
    let mut out: Vec<MoveFormat> = leading
        .map(|comments| MoveFormat {
            comments: Some(comments),
            ..Default::default()
        })
        .into_iter()
        .chain([first])
        .collect();
    loop {
        let skipped = skip_interruptions(input);
        // `many1` stops on `Error` and throws anything else back. Swallowing a
        // `Failure` here would drop the rest of the record without a word,
        // which is the failure this branch exists to remove.
        match move_with_comments(start, Some(side), skipped) {
            Ok((rest, (_, mf))) => {
                side = next_side(&mf, side);
                out.push(mf);
                input = rest;
            }
            // `input` stays before the skipped lines: what ends a run is
            // usually the blank line before a `変化：` block, and the caller
            // needs to see it.
            Err(nom::Err::Error(_)) => break,
            Err(err) => return Err(err),
        }
    }
    let (input, _) = opt(skippable_line_except_a_branch_header)(input)?;
    Ok((input, (first_ply, out)))
}

fn main_moves(start: Color, input: &str) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    let (input, comments) = opt(many1(move_comment_line))(input)?;
    let (input, run) = match moves_with_index(start, input) {
        Ok((rest, (_, v))) => (rest, v),
        Err(nom::Err::Error(_)) => (input, Vec::new()),
        Err(err) => return Err(err),
    };
    Ok((
        input,
        [
            vec![MoveFormat {
                comments,
                ..Default::default()
            }],
            run,
        ]
        .concat(),
    ))
}

fn entire_moves(start: Color, input: &str) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    /// Where in `run` the node at ply `at` sits, `base` being the ply of the
    /// run's first numbered line.
    ///
    /// The ply is the number on the line, not the position in the array. A node
    /// carrying only comments takes no number (`occupies_a_ply`), so counting
    /// array slots instead puts a `変化：N手` block one move off for every such
    /// node before it. The writer numbers the same way
    /// (`converter/kif.rs::write_move_lines`), and a reader that counts
    /// differently reads back a tree the writer did not write.
    fn index_of_ply(run: &[MoveFormat], base: usize, at: usize) -> Option<usize> {
        let mut ply = base;
        for (index, mf) in run.iter().enumerate() {
            if !mf.occupies_a_ply() {
                continue;
            }
            if ply == at {
                return Some(index);
            }
            ply += 1;
        }
        None
    }

    /// Hangs `fork`, which starts at ply `at`, off `run`, whose first numbered
    /// line is ply `base`.
    ///
    /// A run is the alternative *to* the move at its own ply (R-JKF-004). A run
    /// numbered past the end of `run` is not an alternative to a move that is
    /// not there: it is `run` carrying on, and that is how tsshogi reads it
    /// (D3 rule 4 — `research/tables/20-fork-merge.md` T5). Dropping it returned
    /// `Ok` with those moves gone and said nothing.
    fn attach(run: &mut Vec<MoveFormat>, base: usize, at: usize, fork: Vec<MoveFormat>) {
        match index_of_ply(run, base, at) {
            Some(index) => match &mut run[index].forks {
                Some(v) => v.push(fork),
                None => run[index].forks = Some(vec![fork]),
            },
            None => run.extend(fork),
        }
    }

    fn merge_forks(
        (mut moves, mut forks): (Vec<MoveFormat>, Vec<(usize, Vec<MoveFormat>)>),
    ) -> Vec<MoveFormat> {
        let mut stack = Vec::new();
        while let Some(fork) = forks.pop() {
            stack.push(fork);
            if let Some((i, last)) = forks.last_mut() {
                while stack.last().is_some_and(|(j, _)| j > i) {
                    if let Some((j, fork)) = stack.pop() {
                        attach(last, *i, j, fork);
                    }
                }
            }
        }
        // The main line opens with the initial position's slot, which takes no
        // number of its own, so its first numbered line is ply 1 (R-JKF-001).
        while let Some((i, fork)) = stack.pop() {
            attach(&mut moves, 1, i, fork);
        }
        moves
    }

    let (input, _) = many0(skippable_line)(input)?;
    let (mut input, main) = main_moves(start, input)?;
    let mut forks = Vec::new();
    // One `変化：N手` header and the run under it per turn. Reading them one at a
    // time is what makes "this header has no moves under it" answerable: a skip
    // that takes headers along the way leaves nothing to ask about, and a branch
    // goes missing without a word.
    loop {
        let (rest, _) = many0(skippable_line_except_a_branch_header)(input)?;
        // Headers in a row, taken as one. KIF does not read the declaration at
        // all — the tree comes from the ply numbers (D3) — so a second `変化：`
        // over the first is a spare line, not a branch that went missing.
        let mut header = None;
        let mut rest = rest;
        loop {
            match branch_header_line(rest) {
                Ok((after, line)) => {
                    header = Some(line);
                    let (after, _) = many0(skippable_line_except_a_branch_header)(after)?;
                    rest = after;
                }
                // The header is there and cannot be read — it ran into the moves
                // under it. That is the branch, gone.
                Err(err @ nom::Err::Failure(_)) => return Err(err),
                // Not a header. A run can still follow: the tree comes from the
                // ply numbers, not from the declaration (D3).
                Err(_) => break,
            }
        }
        match moves_with_index(start, rest) {
            Ok((after_run, run)) => {
                forks.push(run);
                input = after_run;
            }
            Err(nom::Err::Error(_)) => {
                // A header with nothing under it but the end of the file, or the
                // next header, is a branch that is gone.
                //
                // Anything else there is a different fault with a different
                // cause, and the leftover-input check (D1) names it and where it
                // is; saying "no moves under it" instead would name a line that
                // has moves under it and a cause that is not the one.
                match header {
                    Some(line) if rest.trim().is_empty() => {
                        return Err(broken_line(line, "a 変化 block with no moves under it"));
                    }
                    // The lines just skipped are accounted for even though no run
                    // followed them. A record trails off into prose — `まで<N>手で…`,
                    // a comment block — and the run parser already swallows one
                    // such line, so stopping before them would accept one
                    // trailing line and call the second one unreadable (D1).
                    _ => {
                        input = rest;
                        break;
                    }
                }
            }
            Err(err) => return Err(err),
        }
    }
    Ok((input, merge_forks((main, forks))))
}

/// Reads a record, and says whether the header block held anything.
///
/// A KIF whose whole content is `手合割：平手` and a file that is not a kifu at
/// all come out as the same record: the preset defaults to 平手 and nothing
/// else is set. Only the reader knows the difference — one consumed a line and
/// the other did not — so it has to say so here (D1, `parse_kif_str`).
pub(crate) fn parse(input: &str) -> IResult<&str, (JsonKifuFormat, bool), VerboseError<&str>> {
    let (rest, mut jkf) = parse_without_moves(SHAPES, input)?;
    let read_header = rest.len() < input.len();
    // The side has to come from the starting position, not the ply parity: a
    // handicap record has White at every odd ply (R-HC-001).
    let start = crate::handicap::starting_side(jkf.initial.as_ref());
    let (input, moves) = entire_moves(start, rest)?;
    jkf.moves.extend(moves);
    Ok((input, (jkf, read_header)))
}

#[cfg(test)]
mod tests {
    use super::*;

    // R-KIF-007: KIF's 反則勝ち says the move *before* it was the foul, so the
    // word alone does not name the offender — whose turn it is does. The upper
    // hand moves first in every handicap (R-HC-001), so reading the side off the
    // ply parity records the wrong player as the one who fouled.
    //
    // This is the only thing the parser's own side-to-move decides: every move's
    // colour is overwritten later from the position, so nothing else here fails
    // when it is wrong.
    #[test]
    fn a_handicap_names_the_right_side_at_a_foul_win() {
        use crate::converter::ToCsa;
        for (handicap, first_move, want) in [
            ("平手", "７六歩(77)", MoveSpecial::SpecialIllegalActionBlack),
            (
                "香落ち",
                "３四歩(33)",
                MoveSpecial::SpecialIllegalActionWhite,
            ),
            (
                "四枚落ち",
                "３四歩(33)",
                MoveSpecial::SpecialIllegalActionWhite,
            ),
        ] {
            let kif = format!(
                "手合割：{handicap}\n手数----指手---------消費時間--\n   1 {first_move}\n   2 反則勝ち\n"
            );
            let jkf =
                crate::parser::parse_kif_str(&kif).unwrap_or_else(|e| panic!("{handicap}: {e}"));
            assert_eq!(
                Some(want),
                jkf.moves.last().and_then(|mf| mf.special),
                "reading {handicap}"
            );
            // R-CSA-007 spells the offender out, so the direction is visible.
            let csa = jkf.try_to_csa_owned().expect("writes CSA");
            let sign = if want == MoveSpecial::SpecialIllegalActionBlack {
                "+"
            } else {
                "-"
            };
            assert!(
                csa.lines().any(|l| l == format!("%{sign}ILLEGAL_ACTION")),
                "{handicap} wrote {csa:?}"
            );
        }
    }

    // KIF numbers an outcome line as a ply of its own, so a record that was
    // interrupted and resumed has more plies than moves and the number stops
    // matching the turn. 反則勝ち names its winner only through whose turn it is
    // (R-KIF-007), so reading the turn off the parity records the wrong player
    // as the one who fouled — and `to_csa` then writes the opposite `%±`.
    #[test]
    fn an_outcome_in_the_middle_does_not_shift_the_turn() {
        // ７六歩 is Black's, so 3三→3四 at ply 3 is White's despite the odd ply.
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ３四歩(33)
   4 反則勝ち
";
        let jkf = crate::parser::parse_kif_str(kif).expect("parses");
        assert_eq!(
            Color::White,
            jkf.moves[3].move_.expect("a move").color,
            "the move after 中断 is White's"
        );
        // White moved last, so Black is to move and 反則勝ち is Black's win —
        // which is a foul *by* White, `%-ILLEGAL_ACTION` (R-CSA-007).
        assert_eq!(
            Some(MoveSpecial::SpecialIllegalActionWhite),
            jkf.moves.last().and_then(|mf| mf.special),
        );
    }

    // R-KIF-007. Every word KIF defines for an outcome, and what it means here.
    //
    // 不戦勝 / 不戦敗 have no counterpart among JKF's fourteen, so this crate
    // extends the enum for them (D1). Reading them as nothing was the worse
    // answer once the reader stopped truncating in silence: they are spec
    // vocabulary, so a valid KIF using one made the whole file an error.
    //
    // Both are defined against Black (the upper hand) outright, not against
    // whoever is to move — unlike 反則勝ち in the same table — so the ply does
    // not change what they mean.
    #[test]
    fn kif_outcome_words() {
        // Ply 2, so it is White to move and 反則勝ち accuses Black.
        for (word, want) in [
            ("投了", Some(MoveSpecial::SpecialToryo)),
            ("中断", Some(MoveSpecial::SpecialChudan)),
            ("千日手", Some(MoveSpecial::SpecialSennichite)),
            ("切れ負け", Some(MoveSpecial::SpecialTimeUp)),
            ("反則負け", Some(MoveSpecial::SpecialIllegalMove)),
            ("反則勝ち", Some(MoveSpecial::SpecialIllegalActionBlack)),
            ("持将棋", Some(MoveSpecial::SpecialJishogi)),
            ("入玉勝ち", Some(MoveSpecial::SpecialKachi)),
            ("詰み", Some(MoveSpecial::SpecialTsumi)),
            ("不詰", Some(MoveSpecial::SpecialFuzumi)),
            ("不戦勝", Some(MoveSpecial::SpecialFusensho)),
            ("不戦敗", Some(MoveSpecial::SpecialFusenpai)),
        ] {
            let line = format!("   2 {word}\n");
            let got = move_line(Color::Black, None, &line)
                .ok()
                .and_then(|(_, (_, mf))| mf.special);
            assert_eq!(want, got, "reading {word}");
        }
    }

    // The other direction of the same table. 反則勝ち loses which side fouled;
    // the reader recovers it from the ply number.
    #[test]
    fn kif_outcome_words_are_written_back() {
        const ALL: [MoveSpecial; 14] = [
            MoveSpecial::SpecialToryo,
            MoveSpecial::SpecialChudan,
            MoveSpecial::SpecialSennichite,
            MoveSpecial::SpecialTimeUp,
            MoveSpecial::SpecialIllegalMove,
            MoveSpecial::SpecialIllegalActionBlack,
            MoveSpecial::SpecialIllegalActionWhite,
            MoveSpecial::SpecialJishogi,
            MoveSpecial::SpecialKachi,
            MoveSpecial::SpecialHikiwake,
            MoveSpecial::SpecialMatta,
            MoveSpecial::SpecialTsumi,
            MoveSpecial::SpecialFuzumi,
            MoveSpecial::SpecialError,
        ];
        for special in ALL {
            let Some(word) = special.kif_word() else {
                // 待った and エラー have no KIF word at all.
                assert!(
                    matches!(
                        special,
                        MoveSpecial::SpecialMatta | MoveSpecial::SpecialError
                    ),
                    "{special:?} has no KIF word"
                );
                continue;
            };
            let line = format!("   2 {word}\n");
            let got = move_line(Color::Black, None, &line)
                .ok()
                .and_then(|(_, (_, mf))| mf.special);
            assert!(
                got.is_some(),
                "{special:?} writes {word}, which cannot be read"
            );
        }
    }

    #[test]
    fn parse_move_time_format() {
        assert!(move_time_format("").is_err());
        assert_eq!(
            Ok((
                "",
                TimeFormat {
                    h: None,
                    m: 0,
                    s: 16
                }
            )),
            move_time_format("0:16")
        );
        assert_eq!(
            Ok((
                "",
                TimeFormat {
                    h: Some(0),
                    m: 0,
                    s: 16
                }
            )),
            move_time_format("00:00:16")
        );
    }

    #[test]
    fn parse_move_move() {
        assert!(move_move("").is_err());
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 7, y: 7 }),
                        to: PlaceFormat { x: 7, y: 6 },
                        piece: Kind::FU,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            )),
            move_move("７六歩(77)")
        );
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(PlaceFormat { x: 3, y: 1 }),
                        to: PlaceFormat { x: 4, y: 2 },
                        piece: Kind::KA,
                        same: None,
                        promote: Some(true),
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            )),
            move_move("４二角成(31)")
        );
    }

    #[test]
    fn parse_move_line() {
        assert!(move_line(Color::Black, None, "").is_err());
        assert_eq!(
            Ok((
                "",
                (
                    1,
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(PlaceFormat { x: 7, y: 7 }),
                            to: PlaceFormat { x: 7, y: 6 },
                            piece: Kind::FU,
                            same: None,
                            promote: Some(false),
                            capture: None,
                            relative: None,
                        }),
                        comments: None,
                        time: Some(Time {
                            now: TimeFormat {
                                h: None,
                                m: 0,
                                s: 16
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 16
                            }
                        }),
                        special: None,
                        forks: None,
                    }
                )
            )),
            move_line(Color::Black, None, "1 ７六歩(77) ( 0:16/00:00:16)\n")
        );
        assert_eq!(
            Ok((
                "",
                (
                    3,
                    MoveFormat {
                        move_: None,
                        comments: None,
                        time: Some(Time {
                            now: TimeFormat {
                                h: None,
                                m: 0,
                                s: 3
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 19
                            }
                        }),
                        special: Some(MoveSpecial::SpecialChudan),
                        forks: None,
                    }
                )
            )),
            move_line(Color::Black, None, "3 中断 ( 0:03/ 0:00:19)\n")
        );
        assert_eq!(
            Ok((
                "",
                (
                    1,
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(PlaceFormat { x: 6, y: 9 }),
                            to: PlaceFormat { x: 7, y: 8 },
                            piece: Kind::KI,
                            same: None,
                            promote: Some(false),
                            capture: None,
                            relative: None,
                        }),
                        time: Some(Time {
                            now: TimeFormat {
                                h: None,
                                m: 0,
                                s: 1
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 1
                            }
                        }),
                        ..Default::default()
                    }
                )
            )),
            move_line(
                Color::Black,
                None,
                "   1 ７八金(69)    (00:01 / 00:00:01)\n"
            )
        )
    }

    #[test]
    fn parse_main_moves() {
        assert_eq!(
            Ok((
                "",
                vec![
                    MoveFormat::default(),
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(PlaceFormat { x: 7, y: 7 }),
                            to: PlaceFormat { x: 7, y: 6 },
                            piece: Kind::FU,
                            same: None,
                            promote: Some(false),
                            capture: None,
                            relative: None,
                        }),
                        comments: None,
                        time: Some(Time {
                            now: TimeFormat {
                                h: None,
                                m: 0,
                                s: 16
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 16
                            }
                        }),
                        special: None,
                        forks: None,
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::White,
                            from: Some(PlaceFormat { x: 3, y: 3 }),
                            to: PlaceFormat { x: 3, y: 4 },
                            piece: Kind::FU,
                            same: None,
                            promote: Some(false),
                            capture: None,
                            relative: None,
                        }),
                        comments: None,
                        time: Some(Time {
                            now: TimeFormat {
                                h: None,
                                m: 0,
                                s: 0
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 0
                            }
                        }),
                        special: None,
                        forks: None,
                    },
                    MoveFormat {
                        move_: None,
                        comments: None,
                        time: Some(Time {
                            now: TimeFormat {
                                h: None,
                                m: 0,
                                s: 3
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 19
                            }
                        }),
                        special: Some(MoveSpecial::SpecialChudan),
                        forks: None,
                    },
                ]
            )),
            main_moves(
                Color::Black,
                &r#"
1 ７六歩(77) ( 0:16/00:00:16)
2 ３四歩(33) ( 0:00/00:00:00)
3 中断 ( 0:03/ 0:00:19)
"#[1..],
            )
        );
        assert_eq!(
            Ok((
                "",
                vec![
                    MoveFormat {
                        comments: Some(vec![String::from("開始局面のコメント")]),
                        ..Default::default()
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(PlaceFormat { x: 2, y: 7 }),
                            to: PlaceFormat { x: 2, y: 6 },
                            piece: Kind::FU,
                            same: None,
                            promote: Some(false),
                            capture: None,
                            relative: None,
                        }),
                        time: Some(Time {
                            now: TimeFormat {
                                s: 1,
                                ..Default::default()
                            },
                            total: TimeFormat {
                                h: Some(0),
                                m: 0,
                                s: 1
                            }
                        }),
                        ..Default::default()
                    },
                ]
            )),
            main_moves(
                Color::Black,
                &r#"
*開始局面のコメント
  1 ２六歩(27) ( 0:01/00:00:01)
"#[1..]
            )
        )
    }

    #[test]
    fn parse_entire_moves() {
        let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)
   2 ８四歩(83)    (00:00 / 00:00:00)
   3 ６八銀(79)    (00:00 / 00:00:00)
   4 ３二金(41)    (00:00 / 00:00:00)
   5 ２六歩(27)    (00:00 / 00:00:00)
   6 ８五歩(84)    (00:00 / 00:00:00)
   7 ７七角(88)    (00:00 / 00:00:00)
   8 ３四歩(33)    (00:00 / 00:00:00)
   9 ７八金(69)    (00:00 / 00:00:00)
  10 ７七角成(22)  (00:00 / 00:00:00)
  11 同　銀(68)    (00:00 / 00:00:00)
  12 ２二銀(31)    (00:00 / 00:00:00)

変化：10手
  10 ３三角(22)    (00:00 / 00:00:00)
  11 ６九玉(59)    (00:00 / 00:00:00)
  12 ４二銀(31)    (00:00 / 00:00:00)
  13 ３六歩(37)    (00:00 / 00:00:00)
  14 ７七角成(33)  (00:00 / 00:00:00)

変化：5手
   5 ７七角(88)    (00:00 / 00:00:00)
   6 ３四歩(33)    (00:00 / 00:00:00)
   7 ４八銀(39)    (00:00 / 00:00:00)
   8 ６二銀(71)    (00:00 / 00:00:00)
   9 ３六歩(37)    (00:00 / 00:00:00)
  10 ８五歩(84)    (00:00 / 00:00:00)
  11 ７八金(69)    (00:00 / 00:00:00)
  12 ７四歩(73)    (00:00 / 00:00:00)

変化：9手
   9 １六歩(17)    (00:00 / 00:00:00)
  10 １四歩(13)    (00:00 / 00:00:00)
  11 ２六歩(27)    (00:00 / 00:00:00)
  12 ４二銀(31)    (00:00 / 00:00:00)
  13 ２二角成(77)  (00:00 / 00:00:00)
  14 同　金(32)    (00:00 / 00:00:00)
  15 ７七銀(68)    (00:00 / 00:00:00)
"#[1..];
        let ret = entire_moves(Color::Black, input);
        let (rest, moves) = ret.expect("failed to parse");
        assert!(rest.is_empty());
        assert_eq!(13, moves.len());
        for (i, m) in moves.iter().enumerate() {
            match i {
                5 => {
                    let forks = m.forks.as_ref().expect("no forks");
                    assert_eq!(1, forks.len());
                    assert_eq!(8, forks[0].len());
                    assert!(forks[0].iter().any(|m| m.forks.is_none()))
                }
                10 => {
                    let forks = m.forks.as_ref().expect("no forks");
                    assert_eq!(1, forks.len());
                    assert_eq!(5, forks[0].len());
                    assert!(forks[0].iter().all(|m| m.forks.is_none()))
                }
                _ => assert!(m.forks.is_none()),
            }
        }
    }

    // A run numbered past the end of the run it belongs to is that run carrying
    // on, not an alternative to a move that is not there (D3 rule 4). Here
    // 変化:5 is three plies past 変化:2, which holds one move.
    //
    // This test used to fix the opposite: that 変化:5 is dropped. Dropping it
    // returns `Ok` with the moves gone, which is the silent loss this reader is
    // being taken apart to remove — and tsshogi appends. Indexing straight into
    // the run is what panicked with `index out of bounds` before `checked_sub`,
    // so neither the panic nor the silent drop is the answer.
    #[test]
    fn a_branch_numbered_past_the_end_carries_the_run_on() {
        let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)
   2 ８四歩(83)    (00:00 / 00:00:00)

変化：2
   2 ８六歩(83)    (00:00 / 00:00:00)

変化：5
   5 投了 ( 0:00/ 0:00:00)
"#[1..];
        let (_, moves) = entire_moves(Color::Black, input).expect("parses");
        let forks = moves[2]
            .forks
            .as_ref()
            .expect("変化:2 should attach at index 2");
        assert_eq!(1, forks.len());
        assert_eq!(
            2,
            forks[0].len(),
            "変化:5 continues 変化:2 rather than vanishing"
        );
        assert_eq!(
            Some(MoveSpecial::SpecialToryo),
            forks[0][1].special,
            "and it is the outcome it was"
        );
    }

    // The same rule on the main line. A record whose ply numbers jump — which
    // is what a reader that stopped on a line it could not place produces —
    // must not lose the moves after the jump.
    #[test]
    fn a_branch_numbered_past_the_main_line_carries_it_on() {
        let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)

変化：9
   9 ３四歩(33)    (00:00 / 00:00:00)
"#[1..];
        let (_, moves) = entire_moves(Color::Black, input).expect("parses");
        assert_eq!(3, moves.len(), "index 0 plus two moves");
        assert!(
            moves.iter().all(|m| m.forks.is_none()),
            "nothing branched: {moves:?}"
        );
    }

    // A `変化：` block can open with a node carrying only comments: JKF allows
    // one (R-JKF-002), KIF has no numbered line for it, and `to_kif` writes it
    // straight after the `変化：N手` header. The reader has to take back what
    // the writer puts out — refusing it made this crate reject its own output,
    // and the KI2 reader produces exactly this shape (`parser::ki2::tests::
    // a_comment_only_node_does_not_shift_the_outcome_ply`).
    //
    // A `&` bookmark in the same place was worse than an error: it was taken
    // for an unreadable line and skipped, so it came back missing with nothing
    // said (R-KIF-011).
    //
    // The ply is the number on the line, not the position in the array, so the
    // comment node must not shift where the block attaches either.
    #[test]
    fn a_branch_can_open_with_a_comment_and_survives_a_kif_round_trip() {
        use crate::converter::ToKif;
        use crate::parser::{parse_ki2_str, parse_kif_str};
        for opening in ["*このあとは", "&しおり"] {
            let ki2 = format!(
                "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断\n\n変化：2手\n{opening}\n△８四歩\nまで2手で反則勝ち\n"
            );
            let jkf = parse_ki2_str(&ki2).unwrap_or_else(|e| panic!("{opening}: {e}"));
            let branch = &jkf.moves[2]
                .forks
                .as_ref()
                .unwrap_or_else(|| panic!("{opening}: no branch"))[0];
            assert_eq!(3, branch.len(), "{opening}: comment, move, outcome");

            let kif = jkf
                .try_to_kif_owned()
                .unwrap_or_else(|e| panic!("{opening}: {e}"));
            let back = parse_kif_str(&kif).unwrap_or_else(|e| {
                panic!("{opening}: this crate cannot read its own KIF: {e}\n{kif}")
            });
            assert_eq!(jkf, back, "{opening}: {kif:?}");
        }
    }

    // The same node must not move the branch either. Here the block that opens
    // with a comment is itself the target of a later `変化：`, so counting array
    // slots instead of ply numbers hangs the second block off the wrong move.
    #[test]
    fn a_comment_node_does_not_shift_where_a_branch_attaches() {
        use crate::parser::parse_kif_str;
        let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２六歩(27)

変化：2
*このあとは
   2 ８四歩(83)
   3 ２六歩(27)

変化：3
   3 ７八金(69)
";
        let jkf = parse_kif_str(kif).expect("parses");
        let outer = &jkf.moves[2].forks.as_ref().expect("a branch at ply 2")[0];
        assert_eq!(
            [false, true, true],
            [0, 1, 2].map(|i| outer[i].occupies_a_ply()),
            "comment node first, then plies 2 and 3: {outer:?}"
        );
        // `変化：3` is an alternative to ply 3, which is `outer[2]` — not
        // `outer[3]`, which does not exist.
        assert!(
            outer[2].forks.is_some(),
            "the inner branch hangs off ply 3: {outer:?}"
        );
    }

    // R-KIF-002: a blank line and a `#` line may sit anywhere in the move list.
    // Ending the run of moves on one left every move after it to be read as a
    // branch of a ply that is not there, and dropped — the record came back
    // with one move and `Ok` (GAP-008). tsshogi reads all three.
    //
    // The blank line is the harder of the two: `not_move_line` used to start on
    // the newline itself and take the line after it as its content, so the move
    // following a blank line was eaten whole.
    #[test]
    fn a_blank_or_program_comment_line_does_not_end_the_move_list() {
        use crate::parser::parse_kif_str;
        const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";
        const TAIL: &str = "   2 ３四歩(33)\n   3 ２六歩(27)\n";
        for middle in ["", "\n", "# メモ\n", "\n# メモ\n\n", "   \n"] {
            let jkf = parse_kif_str(&format!("{HEAD}{middle}{TAIL}"))
                .unwrap_or_else(|e| panic!("{middle:?} was rejected: {e}"));
            assert_eq!(3, jkf.moves.len() - 1, "{middle:?}");
        }
    }

    // The blank line before a `変化：` block is what ends the run, so tolerating
    // blank lines inside a run must not swallow the block that follows one. The
    // two readings of a blank line compete: the run skips it and carries on,
    // and the branch reader needs it to have ended the run. What settles it is
    // what comes after — a numbered move line continues the run, a `変化：`
    // header does not.
    #[test]
    fn a_blank_line_still_lets_a_branch_start() {
        use crate::parser::parse_kif_str;
        const HEAD: &str =
            "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ３四歩(33)\n";
        const BRANCH: &str = "\n変化：2\n   2 ８四歩(83)\n";

        for (middle, main_line) in [("", 2), ("\n   3 ２六歩(27)\n", 3)] {
            let jkf = parse_kif_str(&format!("{HEAD}{middle}{BRANCH}"))
                .unwrap_or_else(|e| panic!("{middle:?} was rejected: {e}"));
            assert_eq!(main_line, jkf.moves.len() - 1, "{middle:?}");
            let forks = jkf.moves[2]
                .forks
                .as_ref()
                .unwrap_or_else(|| panic!("no branch at ply 2 for {middle:?}"));
            assert_eq!(1, forks.len(), "{middle:?}");
            assert_eq!(1, forks[0].len(), "the branch holds its one move");
        }
    }

    // A text file need not end with a newline, and kifu written by hand or by
    // other software turn up without one. Requiring one dropped whatever was on
    // the last line — a move, a comment, or `まで<N>手で…` — and returned `Ok`,
    // so the record came back one line short with nothing said about it
    // (R-REQ-004).
    #[test]
    fn the_last_line_is_read_without_a_trailing_newline() {
        use crate::parser::parse_kif_str;
        const HEAD: &str = "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n";

        let moves = parse_kif_str(&format!("{HEAD}   2 ３四歩(33)")).expect("parses");
        assert_eq!(
            2,
            moves.moves.len() - 1,
            "the second move is on the last line"
        );

        let comment = parse_kif_str(&format!("{HEAD}*memo")).expect("parses");
        assert_eq!(
            Some(&vec![String::from("memo")]),
            comment.moves[1].comments.as_ref(),
            "the comment is on the last line"
        );

        let outcome = parse_kif_str(&format!("{HEAD}   2 投了")).expect("parses");
        assert_eq!(
            Some(MoveSpecial::SpecialToryo),
            outcome.moves[2].special,
            "the outcome is on the last line"
        );
    }
}
