use super::kakinoki::{
    blank_line, broken_line, comments_on_the_initial_position, ends_here, fork_header_ply,
    is_reportable_empty_block, move_comment_line, not_move_line, numbered_line, opens_fork_header,
    opens_numbered_line, opens_shared_line, parse_without_moves, LineShapes, NumberedBody,
    WhereABoardCouldBe,
};
use crate::jkf::*;
use crate::notation::is_padding;
use nom::branch::alt;
use nom::combinator::opt;
use nom::error::{ParseError, VerboseError};
use nom::multi::{many0, many1};

use nom::IResult;

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
    opens_line: opens_kif_line,
};

fn opens_kif_line(head: &str) -> bool {
    opens_shared_line(head) || opens_numbered_line(head)
}

/// The `変化：<N>手` line that opens a fork.
///
/// The number is read only to be thrown away: the tree comes from the ply
/// numbers on the move lines, not from this declaration (D3). The line is still
/// matched rather than left to [`not_move_line`], because a line the reader has
/// no shape for is skipped whole — so a `変化：` joined with the move under it
/// takes a whole fork with it, and the record comes back `Ok` with one fewer.
/// Returns the line itself, which is where an error about the fork has to
/// point.
fn fork_header_line(input: &str) -> IResult<&str, &str, VerboseError<&str>> {
    // The number itself is not used: KIF builds the tree from the ply numbers on
    // the move lines (D3). It is read anyway so that this consumes exactly what
    // `opens_fork_header` counts.
    let (rest, _) = fork_header_ply(input)?;
    let (rest, _) = ends_here(SHAPES, input, rest)?;
    Ok((rest, input))
}

/// A line above the main line: blank, a `変化：<N>手` header, or one the move
/// list has no shape for — `まで<N>手で<結末>`, the `手数----指手---` rule, a
/// closing remark.
///
/// Above it, and only there, because a `変化：` there is nothing to act on: a
/// fork cannot come before the moves it is an alternative to, and one written
/// there anyway is what tsshogi skips as well.
fn skippable_line_above_the_main_line(
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, &str, VerboseError<&str>> {
    alt((blank_line, fork_header_line, |input| {
        not_move_line(where_a_board_could_be, input)
    }))(input)
}

/// The same, except that a `変化：` line is left where it is.
///
/// [`entire_moves`] is the one that reads fork headers, because it is the one
/// that knows whether a run follows. A line that merely looks like one is
/// declined too: a header the reader refuses ([`fork_header_line`]) has to
/// reach that loop to be refused, not be skipped as unreadable prose.
fn skippable_line_between_runs(
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, &str, VerboseError<&str>> {
    if opens_fork_header(input) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Not,
        )));
    }
    alt((blank_line, |input| {
        not_move_line(where_a_board_could_be, input)
    }))(input)
}

/// Skips the blank lines and `#` lines that may sit between two moves of a run
/// (R-KIF-002). Ending the run on one leaves every move after it to be read as
/// a fork of a ply that does not exist, and dropped (GAP-008).
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
            let trimmed = input.trim_start_matches(is_padding);
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

// The move parsers below take `(start, input)` and are applied directly rather
// than handing back a parser value, because each needs `start` — whose turn ply
// 1 is — and none of them is used inside a combinator.
//
/// Reads one `<ply> <move>` line.
///
/// `known_side` is whose turn this line is when the run has already been read
/// far enough to know. KIF numbers a special move line as a ply of its own, so a
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
    // What the line says is read once, by the shared reader — the same call the
    // skip's predicate asks (`opens_numbered_line`), so a line this can take
    // is a line no skip is allowed to drop.
    let (input, (i, body, time)) = numbered_line(input)?;
    // The ply decides whose turn it is, and 反則勝ち means the *other* player
    // committed the foul, so the colour is put on afterwards.
    let side_to_move = known_side.unwrap_or_else(|| crate::handicap::side_to_move_at_ply(start, i));
    let mut mf = match body {
        NumberedBody::Special(word) => MoveFormat {
            special: MoveSpecial::from_kif_word(word, side_to_move),
            ..Default::default()
        },
        NumberedBody::Move {
            to,
            piece,
            promote,
            from,
        } => MoveFormat {
            move_: Some(MoveMoveFormat {
                color: side_to_move,
                from,
                // (0, 0) where the move says 同: the normalizer fills it in
                // from the move before.
                to: to.unwrap_or_default(),
                piece,
                same: if to.is_none() { Some(true) } else { None },
                // R-KIF-006: a KIF does not write 不成, so on a move with an
                // origin the absence of `成` is the record saying the move did
                // not promote — not the record saying nothing. Left empty it
                // looks like the latter, and the normalizer then reads the
                // piece name to decide (R-CSA-007, which is how CSA states a
                // promotion) and can put a `成` into a record that has none.
                //
                // A drop has nothing to say either way: a piece enters the
                // board unpromoted, and R-NOT-005 has no 成/不成 for it.
                promote: from.map(|_| promote.unwrap_or(false)),
                capture: None,
                relative: None,
            }),
            ..Default::default()
        },
    };
    // R-KIF-005 / R-KIF-008 say what a move line is made of — the ply, the move,
    // and the time that may or may not follow it — and say nothing about what
    // may come after. So what may come after is whatever is not a line: reading
    // to the end and throwing it away takes the line underneath with it when the
    // newline between them is lost, and a move goes missing from a record that
    // still comes back `Ok`. `ends_here` draws that line.
    let (input, _) = ends_here(SHAPES, line, input)?;
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
    where_a_board_could_be: WhereABoardCouldBe,
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
    // Whose turn the next line is. Only a move passes the turn; a special move takes
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
        // which is the failure this match arm exists to remove.
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
    let where_a_board_could_be = where_a_board_could_be.after(&out);
    let (input, _) =
        opt(|input| skippable_line_between_runs(where_a_board_could_be, input))(input)?;
    Ok((input, (first_ply, out)))
}

fn main_moves(
    start: Color,
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    let (input, comments) = opt(many1(move_comment_line))(input)?;
    let (input, run) = match moves_with_index(start, where_a_board_could_be, input) {
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

fn entire_moves(
    start: Color,
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    /// Where in `run` the node at ply `at` sits, `base` being the ply of the
    /// run's first numbered line.
    ///
    /// The ply is the number on the line, not the position in the array. A node
    /// carrying only comments takes no number (`occupies_ply`), so counting
    /// array slots instead puts a `変化：N手` block one move off for every such
    /// node before it. The writer numbers the same way
    /// (`converter/kif.rs::write_move_lines`), and a reader that counts
    /// differently reads back a tree the writer did not write.
    fn index_of_ply(run: &[MoveFormat], base: usize, at: usize) -> Option<usize> {
        let mut ply = base;
        for (index, mf) in run.iter().enumerate() {
            if !mf.occupies_ply() {
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

    let (input, _) =
        many0(|input| skippable_line_above_the_main_line(where_a_board_could_be, input))(input)?;
    let (mut input, main) = main_moves(start, where_a_board_could_be, input)?;
    let where_a_board_could_be = where_a_board_could_be.after(&main);
    let mut forks = Vec::new();
    // One `変化：N手` header and the run under it per turn. Reading them one at a
    // time is what makes "this header has no moves under it" answerable: a skip
    // that takes headers along the way leaves nothing to ask about, and a fork
    // goes missing without a word.
    loop {
        let (rest, _) =
            many0(|input| skippable_line_between_runs(where_a_board_could_be, input))(input)?;
        // Headers in a row, taken as one. KIF does not read the declaration at
        // all — the tree comes from the ply numbers (D3) — so a second `変化：`
        // over the first is a spare line, not a fork that went missing.
        let mut header = None;
        let mut rest = rest;
        loop {
            match fork_header_line(rest) {
                Ok((after, line)) => {
                    header = Some(line);
                    let (after, _) =
                        many0(|input| skippable_line_between_runs(where_a_board_could_be, input))(
                            after,
                        )?;
                    rest = after;
                }
                // The header is there and cannot be read — it ran into the moves
                // under it. That is the fork, gone.
                Err(err @ nom::Err::Failure(_)) => return Err(err),
                // Not a header. A run can still follow: the tree comes from the
                // ply numbers, not from the declaration (D3).
                Err(_) => break,
            }
        }
        match moves_with_index(start, where_a_board_could_be, rest) {
            Ok((after_run, run)) => {
                forks.push(run);
                input = after_run;
            }
            Err(nom::Err::Error(_)) => {
                // A header with nothing under it but the end of the file, or the
                // next header, is a fork that is gone.
                //
                // Anything else there is a different fault with a different
                // cause, and the leftover-input check (D1) names it and where it
                // is; saying "no moves under it" instead would name a line that
                // has moves under it and a cause that is not the one.
                match header {
                    // `変化：2手を参照` is a note *about* a fork — nothing was
                    // ever under it to lose — while `変化：2手` alone is a fork
                    // that went missing. Both are declarations (D20); what tells
                    // them apart is whether the line says anything else (D1).
                    Some(line) if rest.trim().is_empty() && is_reportable_empty_block(line) => {
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
    let (rest, (mut jkf, header_comments, where_a_board_could_be, read_header)) =
        parse_without_moves(SHAPES, input)?;
    // The side has to come from the initial position, not the ply parity: a
    // handicap record has White at every odd ply (R-HC-001).
    let start = crate::handicap::starting_side(jkf.initial.as_ref());
    let (input, moves) = entire_moves(start, where_a_board_could_be, rest)?;
    jkf.moves.extend(moves);
    comments_on_the_initial_position(header_comments, &mut jkf.moves);
    Ok((input, (jkf, read_header)))
}

#[cfg(test)]
mod tests;
