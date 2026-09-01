use super::kakinoki::{
    an_empty_block_here_is_worth_reporting, branch_header_ply, broken_line,
    comments_on_the_starting_position, ends_here, is_padding, move_comment_line, move_to,
    not_move_line, opens_a_shared_line, parse_without_moves, piece_kind, program_comment_line,
    LineShapes, Position, NOTE_MARKERS, SIDE_MARKS,
};
use crate::jkf::*;
use crate::notation::LINE_ENDS;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::bytes::complete::take_while;
use nom::character::complete::line_ending;
use nom::combinator::{map, opt, value};
use nom::error::{ParseError, VerboseError};
use nom::multi::{many0, many1};
use nom::sequence::{preceded, tuple};
use nom::IResult;

fn side_mark(input: &str) -> IResult<&str, Color, VerboseError<&str>> {
    for (mark, color) in SIDE_MARKS {
        if let Some(rest) = input.strip_prefix(mark) {
            return Ok((rest, color));
        }
    }
    Err(nom::Err::Error(VerboseError::from_error_kind(
        input,
        nom::error::ErrorKind::Alt,
    )))
}

fn single_move(input: &str) -> IResult<&str, MoveFormat, VerboseError<&str>> {
    map(
        tuple((
            side_mark,
            move_to,
            piece_kind,
            // R-NOT-001: the relative part comes before the promotion suffix
            // (`５四角右成`). Reading promotion first leaves the suffix behind,
            // which ends the move list and silently drops the rest of the file.
            opt(alt((
                value(Relative::LU, tag("左上")),
                value(Relative::LM, tag("左寄")),
                value(Relative::LD, tag("左引")),
                value(Relative::RU, tag("右上")),
                value(Relative::RM, tag("右寄")),
                value(Relative::RD, tag("右引")),
                value(Relative::L, tag("左")),
                value(Relative::C, tag("直")),
                value(Relative::R, tag("右")),
                // R-NOT-006 / R-KI2-005: 飛・角の「上」は紙面で「行」と書かれる。
                // 書き出しは正規形だけを使い、読み取りでは揺れを受ける。
                value(Relative::U, alt((tag("上"), tag("行")))),
                value(Relative::M, tag("寄")),
                value(Relative::D, tag("引")),
                value(Relative::H, tag("打")),
            ))),
            // R-NOT-005: 不成 is written out, unlike KIF (R-KIF-006).
            // R-NOT-006 / R-KI2-005: 生 is how 不成 appears in print.
            opt(alt((
                value(false, alt((tag("不成"), tag("生")))),
                value(true, tag("成")),
            ))),
            preceded(
                tuple((take_while(is_padding), opt(line_ending))),
                opt(many1(move_comment_line)),
            ),
        )),
        |(c, to, kind, relative, promote, comments)| {
            // KI2 carries no origin. 打 says the move is a drop, and JKF states
            // that by leaving `from` out (R-JKF-003). Everything else needs the
            // origin looked up against the position, which JKF has no way to
            // ask for — hence the sentinel `crate::normalizer::ORIGIN_UNSTATED`.
            let from = if relative == Some(Relative::H) {
                None
            } else {
                Some(crate::normalizer::ORIGIN_UNSTATED)
            };
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: c,
                    from,
                    to: to.unwrap_or_default(), // Might be (0, 0) if it's the same place as previous
                    piece: kind,
                    same: if to.is_none() { Some(true) } else { None },
                    promote,
                    capture: None,
                    relative,
                }),
                comments,
                ..Default::default()
            }
        },
    )(input)
}

/// What a KI2 line looks like. Both questions take a run of moves that reaches
/// the end as proof of one, and each says in its own doc what else it wants.
pub(super) const SHAPES: LineShapes = LineShapes {
    carries_a_line: a_header_value_carrying_moves,
    opens_a_line: opens_a_ki2_line,
};

/// What a KI2 line looks like: the shapes both formats have, or a run of moves.
fn opens_a_ki2_line(head: &str) -> bool {
    opens_a_shared_line(head) || a_move_run_fills_the_line(head)
}

/// Whether `head` is a run of moves and nothing else, to the end of its line.
///
/// Not just the `▲`: those two characters are how commentary marks a side
/// (`▲有利`, `（△の反撃）`) and how the standard `消費時間` header spells each
/// side's clock. And not just one move either, because the lines this is asked
/// about carry notes. `まで<N>手で<結末>` is where D18 puts them
/// (`まで2手で中断 （▲有利）`) and `変化：N手` takes one the same way, and a note
/// naming the move it is about (`まで114手で先手の勝ち　△３三桂が敗着`) is prose
/// with a move at the head of it. Refusing those refuses records that read as
/// `.kif`.
///
/// So the same line [`a_header_value_carrying_moves`] draws: what a lost newline
/// leaves behind is the line below it, whole, and a line of KI2 is moves all the
/// way to its end. A note stops being one.
fn a_move_run_fills_the_line(head: &str) -> bool {
    if !head.starts_with(|c| SIDE_MARKS.iter().any(|(mark, _)| *mark == c)) {
        return false;
    }
    // `single_move` reads past a newline into the comments under a move, so the
    // question has to be put to this line alone: the line below is not what a
    // line below is made of.
    let line = head.split(LINE_ENDS).next().unwrap_or(head);
    matches!(
        many1(single_move)(line),
        Ok((rest, _)) if rest.trim_start_matches(is_padding).is_empty()
    )
}

/// Whether a header value carries a run of KI2 moves, which means it swallowed
/// the line under it.
///
/// A header value is free text a user can put anything in (R-KIF-004), so unlike
/// [`ends_here`] there is no point at which the line
/// ought to have ended: the question is what the text carries, not where it
/// stops. A KI2 record whose starting position lost its newline puts the whole
/// game in one.
///
/// Two moves in a row that run to the end of the value, because that is what a
/// swallowed line looks like: the header took the rest of its line, and the rest
/// of that line was the moves.
///
/// Neither half of that is enough on its own. `▲` and `△` are how the standard
/// `消費時間` header spells each side's clock (`消費時間：104▲379△380`,
/// `data/tests/kif/oui202106290101.kif`) and one move is how a `戦型` names an
/// opening; and a note that quotes an opening and then says something about it
/// (`序盤は▲７六歩 △３四歩 の出だし`) is a value this crate's own writer produces
/// from a `header` the consumer filled in — refusing it would mean writing files
/// this reader rejects.
///
/// A record whose whole game is one move, or a note that ends on its second
/// move, is still missed or refused (`research/90-gaps.md` GAP-020). Free text
/// and a lost newline are the same characters; this is where the line is drawn.
///
/// A run that reaches the end has to start at one of the marks, so the marks are
/// asked from the last one backwards and the first answer of yes settles it. Not
/// every mark: the value this check exists for is a record that put its whole
/// game on one line, and reading the whole value once per mark made 41 KB of KI2
/// take 19.6 s inside the scan the consumer runs over a whole directory at
/// start-up (R-REQ-002).
///
/// A bounded number of them (`ATTEMPTS` below), because the marks are not all move starts. A move reads
/// the comments under it as its own ([`single_move`]), so the last move of a run
/// swallows a `*` line and any mark inside it; a run of two then begins three
/// marks from the end rather than two. It also means a value with a long enough
/// note behind its moves is missed — `research/90-gaps.md` GAP-020 records what
/// that costs.
fn a_header_value_carrying_moves(value: &str) -> bool {
    /// Enough for the shapes that turn up: a run of two, and a note behind it
    /// holding marks of its own.
    const ATTEMPTS: usize = 8;
    value
        .char_indices()
        .rev()
        .filter(|(_, c)| SIDE_MARKS.iter().any(|(mark, _)| mark == c))
        .take(ATTEMPTS)
        .any(|(i, _)| match many1(single_move)(&value[i..]) {
            Ok((rest, moves)) => moves.len() >= 2 && rest.trim().is_empty(),
            Err(_) => false,
        })
}

/// Reads the `まで<N>手で…` line that KI2 uses instead of an outcome move.
///
/// `ply` is the ply the outcome occupies, which decides whose turn it is and
/// therefore which side 反則勝ち accuses.
fn end_of_game_line(
    start: Color,
    ply: usize,
) -> impl FnMut(&str) -> IResult<&str, MoveFormat, VerboseError<&str>> {
    move |input| {
        let (line, _) = many0(line_ending)(input)?;
        // `line` still points at the start of the `まで…` line. `nom` reports a
        // position, so an error built after the line is consumed points at the
        // *next* one and shows a blank caret.
        let (input, _) = tag("まで")(line)?;
        // `まで` may be followed by `<N>手で`; the ply is already known from the
        // moves that were read, so the number is not needed. Only when it is on
        // this line — `split_once` would otherwise reach into the next one.
        let phrase = match input.split_once("手で") {
            Some((head, rest)) if !head.contains(LINE_ENDS) => rest,
            _ => input,
        };
        let phrase = phrase.trim_start_matches(is_padding);
        // The outcome word owns the line up to the first space, note marker or
        // line ending. This line is the only place KI2 has to put an outcome
        // (R-KI2-006, D5), and it is also where a note about the game goes
        // (`まで2手で中断 （▲有利）`, D18) — so the word ends where the note
        // begins. What opens a note is D17's table, the same one the line-end
        // rule uses, and what is left goes to that rule below.
        let end = phrase
            .find(|c: char| is_padding(c) || LINE_ENDS.contains(&c) || NOTE_MARKERS.contains(&c))
            .unwrap_or(phrase.len());
        let side_to_move = crate::handicap::side_to_move_at_ply(start, ply);
        match MoveSpecial::from_ki2_phrase(&phrase[..end], side_to_move) {
            // What follows the word goes through the same check as any other
            // line end (D17): a note is skipped, and the line below it — a
            // `変化：` header, more moves, a comment — is the record telling us
            // its newline is gone.
            Some(special) => {
                let (input, _) = ends_here(SHAPES, line, &phrase[end..])?;
                Ok((
                    input,
                    MoveFormat {
                        special: Some(special),
                        ..Default::default()
                    },
                ))
            }
            // `まで` matched, so this line *is* the outcome line whatever it
            // says. Reporting a recoverable error would leave the line
            // unconsumed, which ends the move list and drops the `変化：`
            // blocks after it without a word (D1: a record we cannot read has
            // to say so). `Failure` is what `opt` does not swallow.
            None => Err(broken_line(
                line,
                "this outcome is not one of the words KI2 has",
            )),
        }
    }
}

/// Reads a `変化：N手` header and returns the ply it names, with the line itself
/// — which is where an error about the block under it has to point (`moves`).
///
/// The declaration `N` is read here and used, unlike in KIF where the tree comes
/// from the ply numbers alone (D3): tsshogi reads this line in KI2 and not in
/// KIF, and the two readers have to build the same tree from the same file.
///
/// The header owns the rest of its line. Reading to the end of the line and
/// dropping what was there takes the first move of the branch with it whenever
/// the newline between them is lost.
fn branch_header(input: &str) -> IResult<&str, (usize, &str), VerboseError<&str>> {
    let (line, _) = many0(line_ending)(input)?;
    let (rest, ply) = branch_header_ply(line)?;
    // KI2 is the reader that uses the number (D3, D19), so it is the one that
    // says when the number cannot be used. Plies count from 1 (R-JKF-001), so
    // `変化：０手` names no move for a branch to be an alternative to; carried
    // on, it reaches `attach_branch`, which cannot find a departure point and
    // returns — taking the branch and everything under it without a word
    // (`research/90-gaps.md` GAP-018). Refused, the line is left for the
    // leftover-input check to report (D1).
    if ply == 0 {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            line,
            nom::error::ErrorKind::Verify,
        )));
    }
    let (rest, _) = ends_here(SHAPES, line, rest)?;
    Ok((rest, (ply, line)))
}

/// Reads one line of the record: the moves and any outcome lines among them.
///
/// An outcome does not end the run. A game that was interrupted and resumed
/// records `中断` in the middle and keeps going, and stopping at the first
/// `まで…` line drops every move after it without saying so.
fn move_run(
    start: Color,
    first_ply: usize,
) -> impl FnMut(&str) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    move |mut input| {
        let mut out = Vec::new();
        loop {
            // R-KI2-002: blank lines sit between runs of moves — the
            // specification's own example is written that way. R-KI2-001: KI2 is
            // "a game record people can read", pasted as-is, so a run can also
            // arrive indented. Stopping at either drops the rest of the file.
            // A `#` line is a note from the program that wrote the file and may
            // sit anywhere (R-KIF-002), so it does not end the run — the same
            // record has to read the same way as `.kif` and as `.ki2` (D10).
            let (rest, _) = many0(alt((
                line_ending,
                nom::bytes::complete::take_while1(is_padding),
                nom::combinator::recognize(program_comment_line),
                // Prose between the header and the moves under it. KIF skips
                // one here as well, and a record that reads as `.kif` has to
                // read as `.ki2` (D18). Everything the reader has a shape for
                // is left where it is.
                nom::combinator::recognize(|input| {
                    a_line_only_prose_opens(Position::PastTheOpeningBlock, input)
                }),
            )))(input)?;
            // A comment before any move of a run belongs to a node of its own:
            // that is what `write_line` produces for a JKF node carrying only
            // comments, and a `変化：` block can open with one.
            let (rest, leading) = opt(many1(move_comment_line))(rest)?;
            let read_comments = leading.is_some();
            out.extend(leading.map(|comments| MoveFormat {
                comments: Some(comments),
                ..Default::default()
            }));
            let (rest, v) = many0(single_move)(rest)?;
            let read_moves = !v.is_empty();
            out.extend(v);
            // The ply an outcome line names counts moves and outcomes, not
            // nodes: a comment-only node writes no line for anyone to number.
            // The writers count the same way (`MoveFormat::occupies_a_ply`).
            let numbered = out.iter().filter(|mf| mf.occupies_a_ply()).count();
            // Saturating because `first_ply` comes from a `変化：N手` line and a
            // record can say any `N` (`branch_header_ply`, D19). The ply is only
            // used to decide whose turn an outcome is, and a branch that far out
            // has no node to attach to anyway — an overflow here would be a
            // panic in a Tauri command (R-REQ-002).
            let ply = first_ply.saturating_add(numbered);
            let (rest, end) = opt(end_of_game_line(start, ply))(rest)?;
            let read_end = end.is_some();
            out.extend(end);
            input = rest;
            if !read_comments && !read_moves && !read_end {
                break;
            }
        }
        Ok((input, out))
    }
}

/// A line the reader has no shape for — prose, and nothing else.
///
/// **The only skip in this reader.** Every shape the reader does have belongs to
/// whoever reads it, not to this skip. A `まで…` is the outcome and [`move_run`]
/// reads it; a `変化：` says a branch starts and [`moves`] reads it; `*` and `&`
/// are comments. Skipping one takes what it said with it, and the record comes
/// back `Ok` without it — an outcome that never happened, or a branch whose
/// moves are read as the main line carrying on (R-JKF-004). Two skips is how
/// that happens: one of them learns a shape and the other goes on swallowing it.
///
/// Narrower than the KIF skip in one more way. KIF puts every move at the head
/// of its own numbered line, so a line that does not start with a digit holds no
/// move. KI2 writes moves anywhere along a line (R-KI2-001), so skipping a whole
/// line can throw moves away — the silent loss D1 exists to remove. A line
/// carrying `▲` or `△` is therefore never skippable, and the leftover-input
/// check reports it instead.
fn a_line_only_prose_opens(
    where_it_is: Position,
    input: &str,
) -> IResult<&str, &str, VerboseError<&str>> {
    // Past the padding, the same as `begins_the_line_below` looks past it. A
    // `変化：` one tab in is still a `変化：`, and asking from the raw start
    // hands it to the skip below — the branch is gone and its moves carry on as
    // the main line (R-JKF-004).
    if opens_a_shared_line(input.trim_start_matches(is_padding)) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Not,
        )));
    }
    let (rest, line) = not_move_line(where_it_is, input)?;
    if line.contains(|c| SIDE_MARKS.iter().any(|(mark, _)| *mark == c)) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }
    Ok((rest, line))
}

/// Attaches `branch` as an alternative to the move at ply `start_ply`.
///
/// `path` is the run of branch choices taken to get here, as
/// `(ply the branch departs from, index among that node's branches)`. KI2 names
/// the departure ply explicitly, and a later `変化：` is read against the branch
/// most recently entered, so the path is first cut back to the branches that
/// start before `start_ply`.
fn attach_branch(
    main: &mut [MoveFormat],
    path: &mut Vec<(usize, usize)>,
    start_ply: usize,
    branch: Vec<MoveFormat>,
) {
    path.retain(|&(ply, _)| ply < start_ply);
    let mut level: &mut [MoveFormat] = main;
    // `level[0]` is ply `level_first_ply`.
    let mut level_first_ply = 1;
    for &(ply, index) in path.iter() {
        let Some(node) = ply
            .checked_sub(level_first_ply)
            .and_then(|i| level.get_mut(i))
        else {
            return;
        };
        let Some(next) = node.forks.as_mut().and_then(|f| f.get_mut(index)) else {
            return;
        };
        level = next;
        level_first_ply = ply;
    }
    let Some(node) = start_ply
        .checked_sub(level_first_ply)
        .and_then(|i| level.get_mut(i))
    else {
        return;
    };
    let forks = node.forks.get_or_insert_with(Vec::new);
    forks.push(branch);
    path.push((start_ply, forks.len() - 1));
}

/// Reads the main line and every `変化：N手` block. `start` is whose turn ply 1
/// is, which the outcome line needs and the ply parity cannot supply.
fn moves(start: Color, input: &str) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    // Everything above the first move, in one loop.
    //
    // Comments belong to the starting position (R-KIF-010) and are kept; the
    // rest is read and dropped. A `変化：` among them names no move to be an
    // alternative to, so there is nothing to act on — KIF does the same
    // (`kif::skippable_line`), and tsshogi does it deliberately: its
    // `kakinoki.mjs` cites shogihome #570 for records whose header block holds
    // one, and says a `変化：` before the moves start must be ignored. Left in
    // place it reaches the branch loop below, which reads the whole game as a
    // branch of a main line that has no node to hang it on — `attach_branch`
    // finds nothing and returns, and the record comes back `Ok` with no moves
    // at all (D1, R-JKF-004).
    //
    // One loop rather than a comment reader and then a gate: what a header block
    // holds between its last keyword and the first move is prose, `#` notes,
    // blank lines and comments in whatever order the writer left them, and a
    // gate that fires in one place is a gate the next line walks around.
    let mut input = input;
    let mut comments = Vec::new();
    loop {
        if let Ok((rest, comment)) = move_comment_line(input) {
            comments.push(comment);
            input = rest;
            continue;
        }
        match alt((
            nom::combinator::recognize(branch_header),
            nom::combinator::recognize(program_comment_line),
            nom::combinator::recognize(|input| {
                a_line_only_prose_opens(Position::WhereABoardCouldStill, input)
            }),
            line_ending,
        ))(input)
        {
            Ok((rest, _)) => input = rest,
            Err(nom::Err::Failure(err)) => return Err(nom::Err::Failure(err)),
            Err(_) => break,
        }
    }
    let comments = (!comments.is_empty()).then_some(comments);
    let (mut input, main) = move_run(start, 1)(input)?;
    let mut out = vec![MoveFormat {
        comments,
        ..Default::default()
    }];
    out.extend(main);
    // `変化：N手` blocks, in the order the file lists them.
    let mut path = Vec::new();
    loop {
        match branch_header(input) {
            Ok((mut rest, (mut start_ply, mut header))) => {
                // Headers in a row are one declaration: the last of them says
                // which ply the branch leaves, and the ones over it are spare
                // lines rather than branches that went missing.
                loop {
                    match branch_header(rest) {
                        Ok((after, (ply, line))) => {
                            rest = after;
                            start_ply = ply;
                            header = line;
                        }
                        // The same as at the top of the loop: a header that ran
                        // into the moves under it is the branch, gone. Taking it
                        // for "no more headers" reports the header above this
                        // one, and says its block is empty when what is broken
                        // is the line below it.
                        Err(err @ nom::Err::Failure(_)) => return Err(err),
                        Err(_) => break,
                    }
                }
                let (rest, branch) = move_run(start, start_ply)(rest)?;
                if branch.is_empty() {
                    // The header says a branch follows. Reading nothing under it
                    // means the branch is gone, and carrying on returns a record
                    // that is a whole branch short without saying so.
                    //
                    // Unless the line says more than the number. `変化：2手を参照`
                    // is a note *about* a branch — nothing was ever under it to
                    // lose — while `変化：2手` alone is a branch that went
                    // missing. Both are branch declarations (D20); what tells
                    // them apart is whether the line says anything else (D1).
                    if an_empty_block_here_is_worth_reporting(header) {
                        return Err(broken_line(header, "a 変化 block with no moves under it"));
                    }
                    input = rest;
                    continue;
                }
                attach_branch(&mut out[1..], &mut path, start_ply, branch);
                input = rest;
                continue;
            }
            // A `Failure` is `branch_header` saying the line *is* a header and is
            // broken — it ran into the moves under it. Taking it for "not a
            // branch header" hands the line to the skip below, which drops the
            // whole block and returns `Ok`.
            Err(err @ nom::Err::Failure(_)) => return Err(err),
            Err(_) => {}
        }
        // A line the format has no shape for — a note after the record, a
        // closing remark — is skipped rather than left behind for the
        // leftover-input check, the same as KIF does: a record readable as
        // `.kif` is readable as `.ki2` (D10). The skip is the one `move_run`
        // uses, because two skips means a shape only one of them knows. `#` is
        // read here rather than skipped because it is a shape (R-KIF-002).
        // `not_move_line` under the skip consumes a line at a time, so this
        // cannot spin.
        match alt((nom::combinator::recognize(program_comment_line), |input| {
            a_line_only_prose_opens(Position::PastTheOpeningBlock, input)
        }))(input)
        {
            Ok((rest, _)) => input = rest,
            Err(_) => break,
        }
    }
    Ok((input, out))
}

/// Reads a record, and says whether the header block held anything.
///
/// A KIF whose whole content is `手合割：平手` and a file that is not a kifu at
/// all come out as the same record: the preset defaults to 平手 and nothing
/// else is set. Only the reader knows the difference — one consumed a line and
/// the other did not — so it has to say so here (D1, `parse_kif_str`).
pub(crate) fn parse(input: &str) -> IResult<&str, (JsonKifuFormat, bool), VerboseError<&str>> {
    let (rest, (mut jkf, header_comments)) = parse_without_moves(SHAPES, input)?;
    let read_header = rest.len() < input.len();
    // The side has to come from the starting position, not the ply parity: a
    // handicap record has White at every odd ply (R-HC-001).
    let start = crate::handicap::starting_side(jkf.initial.as_ref());
    let (input, moves) = moves(start, rest)?;
    jkf.moves.extend(moves);
    comments_on_the_starting_position(header_comments, &mut jkf.moves);
    Ok((input, (jkf, read_header)))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::normalizer::ORIGIN_UNSTATED;
    use std::collections::HashMap;

    // R-NOT-001: <到達地点><駒種><相対位置><動作><成/不成>. Reading the promotion
    // suffix before the relative part leaves it unconsumed, which ends the move
    // list and drops the rest of the file without an error.
    #[test]
    fn single_move_reads_relative_before_promotion() {
        for (input, relative, promote) in [
            ("▲５四角右成", Some(Relative::R), Some(true)),
            ("▲５四角左不成", Some(Relative::L), Some(false)),
            ("▲３三銀右上成", Some(Relative::RU), Some(true)),
            ("▲５四角成", None, Some(true)),
            ("▲５四角右", Some(Relative::R), None),
            ("▲５四角", None, None),
            // R-NOT-006 / R-KI2-005: the spellings print uses.
            ("▲５四角行", Some(Relative::U), None),
            ("▲５四角生", None, Some(false)),
        ] {
            let (rest, mf) = single_move(input).unwrap_or_else(|e| panic!("{input}: {e:?}"));
            assert_eq!("", rest, "unconsumed input for {input}");
            let mv = mf.move_.expect("a move");
            assert_eq!(relative, mv.relative, "relative for {input}");
            assert_eq!(promote, mv.promote, "promote for {input}");
        }
    }

    // The example printed in the KI2 specification itself (R-KI2-002), copied
    // verbatim. It has blank lines between runs of moves, which is exactly what
    // the specification points out about the format. Stopping at the first one
    // reads 6 of the 12 moves and returns `Ok`.
    #[test]
    fn the_specification_example_reads_every_move() {
        let ki2 = "開始日時：1999/04/08
終了日時：1999/04/09
棋戦：第５７期名人戦７番勝負 第１局
戦型：横歩取り
先手：谷川浩司 九段
後手：佐藤康光 名人

▲７六歩 △３四歩 ▲２六歩 △８四歩 ▲２五歩
△８五歩

▲７八金 △３二金 ▲２四歩 △同　歩 ▲同　飛
△８六歩
";
        let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
        let moves = jkf.moves[1..]
            .iter()
            .filter(|mf| mf.move_.is_some())
            .count();
        assert_eq!(12, moves, "read {moves} of 12: {:?}", jkf.moves);
    }

    // R-KI2-001: KI2 is "a game record people can read", and the specification
    // says a run of moves pasted out of one is readable too. Pasted text arrives
    // indented, and stopping at the indent drops every move after it.
    #[test]
    fn an_indented_run_of_moves_is_read() {
        let ki2 = "▲７六歩 △３四歩\n  ▲２六歩 △８四歩\n";
        let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
        let moves = jkf.moves[1..]
            .iter()
            .filter(|mf| mf.move_.is_some())
            .count();
        assert_eq!(4, moves, "read {moves} of 4: {:?}", jkf.moves);
    }

    // A comment can open a `変化：` block — that is what this crate's own writer
    // produces for a JKF branch whose first node carries only comments. Failing
    // to read it ends the move list and throws away every block after it.
    #[test]
    fn a_branch_may_open_with_a_comment() {
        let ki2 = "▲７六歩 △３四歩

変化：2手
*この分岐の狙い
△８四歩

変化：2手
△４四歩
";
        let jkf = crate::parser::parse_ki2_str(ki2).expect("parses");
        let forks = jkf.moves[2].forks.as_ref().expect("branches at ply 2");
        assert_eq!(2, forks.len(), "both blocks survive: {:?}", jkf.moves);
        assert_eq!(
            Some(&vec!["この分岐の狙い".to_owned()]),
            forks[0][0].comments.as_ref(),
            "the comment is kept"
        );
    }

    // Leaving a `まで…` line unconsumed ends the move list and throws away
    // every `変化：` block after it while still returning `Ok`. The outcome word
    // is the one thing a reader cannot recover from the rest of the file, so a
    // record that says something we do not understand has to be reported (D1).
    #[test]
    fn an_unreadable_outcome_line_is_an_error_not_a_silent_truncation() {
        for phrase in ["持将棋成立", "先手の不戦敗", "中座", "引き分け"] {
            let ki2 = format!(
                "▲７六歩 △３四歩\nまで2手で{phrase}\n\n変化：2手\n△８四歩\n\n変化：1手\n▲２六歩\n"
            );
            let err = crate::parser::parse_ki2_str(&ki2)
                .err()
                .unwrap_or_else(|| panic!("{phrase} was accepted"));
            assert!(
                matches!(err, crate::error::ParseError::Ki2(_)),
                "{phrase} gave {err:?}"
            );
            // The one thing the reader cannot recover is the word it could not
            // read, so the error has to carry it and point at its own line.
            let text = err.to_string();
            assert!(text.contains(phrase), "{phrase} is missing from {text:?}");
            assert!(
                text.contains("at line 2"),
                "{phrase} should point at line 2: {text:?}"
            );
        }
    }

    // GAP-017: the ply an outcome line names counts moves and outcomes, not
    // nodes — a comment-only node writes no line for anyone to number. Counting
    // nodes instead shifts the outcome one ply on, and the side to move with
    // it: a bare `反則勝ち` names its winner only through whose turn it is
    // (D5), so the record comes back accusing the other player.
    //
    // A comment-only node is a shape the KI2 *writer* produces for a JKF node
    // carrying only comments, and a `変化：` block can open with one — which is
    // where the reader meets it, since a comment after a move attaches to that
    // move instead. So the round trip is what exercises the count.
    #[test]
    fn a_comment_only_node_does_not_shift_the_outcome_ply() {
        use crate::converter::ToKi2;
        use crate::parser::parse_ki2_str;
        const KI2: &str = "手合割：平手
▲７六歩 △３四歩
まで2手で中断

変化：2手
*このあとは
△８四歩
まで2手で反則勝ち
";
        let jkf = parse_ki2_str(KI2).expect("parses");
        let branch = &jkf.moves[2].forks.as_ref().expect("a branch at ply 2")[0];
        assert_eq!(3, branch.len(), "comment node, move, outcome: {branch:?}");
        assert_eq!(
            Some(vec![String::from("このあとは")]),
            branch[0].comments,
            "the branch opens with a node that is only a comment"
        );
        // Ply 2 is the branch's move, so the outcome takes ply 3 — Black's.
        // R-CSA-007: `+ILLEGAL_ACTION` is Black fouling, so White wins.
        assert_eq!(
            Some(MoveSpecial::SpecialIllegalActionWhite),
            branch[2].special
        );

        // And again through the writer, which is where the shape comes from.
        let written = jkf.try_to_ki2_owned().expect("writes KI2");
        let back = parse_ki2_str(&written).expect("reads back");
        assert_eq!(jkf, back, "{written:?}");
    }

    // Only prose is skipped. A line this reader has a shape for belongs to
    // whoever reads it: a `まで…` is the outcome (KI2 has no other way to record
    // one, R-KI2-006 / D5), and a `変化：` says a branch starts — including a
    // spelling this reader cannot read, which the leftover-input check then
    // reports (D1) rather than hiding a branch inside the main line (R-JKF-004).
    #[test]
    fn a_line_the_reader_has_a_shape_for_is_never_skipped() {
        use crate::parser::parse_ki2_str;
        const MOVES: &str = "手合割：平手\n▲７六歩 △３四歩\n";
        for before in ["# Kifu for Windows V7.30\n", "感想戦\n", "\n"] {
            let jkf = parse_ki2_str(&format!("{MOVES}{before}まで2手で投了\n"))
                .unwrap_or_else(|e| panic!("{before:?}: {e}"));
            assert!(
                jkf.moves.last().expect("a node").special.is_some(),
                "{before:?}: the outcome is gone"
            );
        }
        // A branch header is one however its number is spelled, and every
        // spelling `opens_a_branch_header` counts is one `branch_header_ply`
        // consumes. A spelling only one of the two knows is a line nobody owns:
        // counted, so not skipped as prose; unreadable, so not consumed.
        for header in ["変化：2手", "変化：２手", "変化： 2手", "変化：２手"] {
            for block in ["▲２六歩\n", "まで2手で投了\n"] {
                let jkf = parse_ki2_str(&format!("{MOVES}{header}\n{block}"))
                    .unwrap_or_else(|e| panic!("{header:?} + {block:?}: {e}"));
                assert!(
                    jkf.moves[2].forks.is_some(),
                    "{header:?} + {block:?}: read as the main line carrying on: {:?}",
                    jkf.moves
                );
            }
        }
        // And `変化：` with no number is a sentence opening with two characters,
        // so it is prose — the same as KIF makes of it (D18).
        let prose = parse_ki2_str(&format!("{MOVES}変化：ここから\n")).expect("reads");
        assert_eq!(2, prose.moves.len() - 1, "{:?}", prose.moves);
    }

    // R-KI2-001: KI2 is a record people read, and what people paste is padded
    // with whatever the place they read it padded with. A separator narrower
    // than `is_padding` leaves the padding inside the outcome word, and the
    // record comes back as an error naming a word that is in the vocabulary.
    // The list is a sample of what `is_padding` answers for, not its definition
    // — a keyboard's three, and what a web page and a typesetter put in.
    #[test]
    fn padding_after_the_outcome_word_is_padding_whichever_space_it_is() {
        use crate::parser::parse_ki2_str;
        for pad in [' ', '\t', '　', '\u{a0}', '\u{2009}', '\u{b}', '\u{c}'] {
            assert!(super::is_padding(pad), "{pad:?}");
            for src in [
                format!("手合割：平手\n▲７六歩 △３四歩\nまで2手で投了{pad}\n"),
                format!("手合割：平手\n▲７六歩 △３四歩\nまで2手で{pad}投了\n"),
            ] {
                let jkf = parse_ki2_str(&src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
                assert!(
                    jkf.moves.last().expect("a node").special.is_some(),
                    "{src:?}: the outcome is gone"
                );
            }
        }
    }

    // Plies count from 1 (R-JKF-001), so `変化：０手` names no move for a branch
    // to be an alternative to. Read as a number it reaches `attach_branch`,
    // which cannot find a departure point and returns — taking the branch and
    // the outcome under it without a word (GAP-018). Refused, the line is what
    // the leftover-input check reports (D1).
    //
    // KIF has to keep reading it: the tree there comes from the ply numbers on
    // the move lines and the declaration is never looked at (D3), so refusing
    // would reject a record over a digit KIF does not read.
    #[test]
    fn a_branch_that_departs_from_no_ply_is_refused_rather_than_dropped() {
        use crate::parser::{parse_ki2_str, parse_kif_str};
        const KI2: &str = "手合割：平手\n▲７六歩 △８四歩 ▲２六歩 △４四歩\n";
        const KIF: &str =
            "手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩(77)\n   2 ８四歩(83)\n\n";
        for zero in ["0", "０"] {
            assert!(
                parse_ki2_str(&format!(
                    "{KI2}変化：{zero}手\n△８四歩 ▲２五歩\nまで6手で中断\n"
                ))
                .is_err(),
                "変化：{zero}手: the branch and the outcome went without a word"
            );
            let kif = parse_kif_str(&format!("{KIF}変化：{zero}手\n   2 ３四歩(33)\n"))
                .unwrap_or_else(|e| panic!("変化：{zero}手 in KIF: {e}"));
            assert_eq!(
                1,
                kif.moves.iter().filter(|m| m.forks.is_some()).count(),
                "変化：{zero}手: KIF builds its tree from the ply numbers (D3)"
            );
        }
        // A ply too large to use is not a spelling that cannot be read: the
        // record still says a branch starts, and D19 puts the ceiling at
        // "a damaged record is not fatal" rather than at supporting the number.
        // The branch itself has no node to hang on, so it goes the way every
        // out-of-range departure ply goes (GAP-018) — that is the part D19
        // accepts, and it is different from `変化：０手`, which is refused
        // because a record cannot mean it.
        for huge in ["18446744073709551616", "99999999999999999999"] {
            let kif = parse_kif_str(&format!("{KIF}変化：{huge}手\n   2 ３四歩(33)\n"))
                .unwrap_or_else(|e| {
                    panic!("変化：{huge}手: refused over a number KIF never reads: {e}")
                });
            assert_eq!(
                1,
                kif.moves.iter().filter(|m| m.forks.is_some()).count(),
                "変化：{huge}手: KIF builds its tree from the ply numbers (D3)"
            );
            let ki2 = parse_ki2_str(&format!("{KI2}変化：{huge}手\n△８四歩\n"))
                .unwrap_or_else(|e| panic!("変化：{huge}手: {e}"));
            assert_eq!(
                0,
                ki2.moves.iter().filter(|m| m.forks.is_some()).count(),
                "変化：{huge}手: KI2 has no node at that ply, so the branch goes (GAP-018)"
            );
        }
    }

    // A `変化：` above the first move names no move to be an alternative to.
    // tsshogi reads such records deliberately (shogihome #570) and ignores the
    // line; KIF has always done the same (`kif::skippable_line`). Left in, it
    // makes the whole game a branch of a main line with no node to hang it on,
    // and `attach_branch` drops it without a word — `Ok`, no moves at all.
    #[test]
    fn a_branch_header_above_the_first_move_is_read_and_dropped() {
        use crate::parser::parse_ki2_str;
        // The header block holds whatever the writer left between its last
        // keyword and the first move, in whatever order, at whatever column. A
        // gate that fires in one place is a gate the next line walks around.
        for colon in ['：', ':'] {
            for before in [
                "",
                "*まえがき\n",
                "先手番\n",
                "解説\n",
                "*a\n解説\n",
                "解説\n*a\n",
                "# note\n",
                "\n",
            ] {
                for pad in ["", " ", "　", "\t"] {
                    let src =
                        format!("先手：A\n{before}{pad}変化{colon}2手\n▲７六歩 △３四歩 ▲２六歩\n");
                    let jkf = parse_ki2_str(&src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
                    assert_eq!(
                        3,
                        jkf.moves.iter().filter(|m| m.move_.is_some()).count(),
                        "{src:?}: the game was read as a branch and dropped"
                    );
                }
            }
        }
        // And the comments among them are still the starting position's, in the
        // order the file wrote them (R-KIF-010).
        let kept =
            parse_ki2_str("先手：A\n*a\n解説\n*b\n変化：2手\n▲７六歩 △３四歩\n").expect("reads");
        assert_eq!(
            Some(&vec![String::from("a"), String::from("b")]),
            kept.moves[0].comments.as_ref()
        );
        // A branch that does name a move is still a branch, and one with an
        // empty block still says so (D1).
        let jkf = parse_ki2_str("手合割：平手\n▲７六歩 △８四歩 ▲２六歩\n変化：3手\n▲２五歩\n")
            .expect("reads");
        assert!(
            jkf.moves.iter().any(|m| m.forks.is_some()),
            "{:?}",
            jkf.moves
        );
        assert!(parse_ki2_str("手合割：平手\n▲７六歩 △８四歩\n変化：2手\n").is_err());
    }

    // The padding between two moves is padding, whichever space it is. The two
    // questions that call `many1(single_move)` themselves — the ones that catch
    // a line whose newline is gone — have to read the same set as the skip in
    // `move_run`, or a run separated by a full-width space stops being a run
    // halfway through and the rest is taken for a note.
    #[test]
    fn moves_are_separated_by_padding_whichever_space_it_is() {
        use crate::parser::parse_ki2_str;
        for pad in [" ", "\t", "　", "\u{a0}", "\u{2009}"] {
            assert!(
                parse_ki2_str(&format!("手合割：平手 ▲７六歩{pad}△３四歩\n")).is_err(),
                "{pad:?}: the game went into the header without a word"
            );
            assert!(
                parse_ki2_str(&format!(
                    "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断 ▲７六歩{pad}△３四歩\n"
                ))
                .is_err(),
                "{pad:?}: the moves after the outcome went without a word"
            );
            let run = parse_ki2_str(&format!("手合割：平手\n▲７六歩{pad}△３四歩\n"))
                .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
            assert_eq!(2, run.moves.len() - 1, "{pad:?}");
        }
    }

    // R-KI2-001: a run can arrive indented, because KI2 is a record people read
    // and paste. The padding in front of a line has to be padding to every
    // reader of that line, not just to the ones that ask `is_padding`: a line
    // whose shape is only recognised from column 0 falls to the prose skip, and
    // what it said goes with it — the outcome KI2 has no other place for
    // (R-KI2-006 / D5), or a branch whose moves then read as the main line
    // carrying on (R-JKF-004).
    #[test]
    fn a_line_is_the_line_it_is_however_far_in_it_starts() {
        use crate::parser::parse_ki2_str;
        const MOVES: &str = "手合割：平手\n▲７六歩 △３四歩\n";
        for pad in ["", " ", "　", "\t", "\u{a0}"] {
            let outcome = parse_ki2_str(&format!("{MOVES}{pad}まで2手で投了\n"))
                .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
            assert!(
                outcome.moves.last().expect("a node").special.is_some(),
                "{pad:?}: the outcome is gone"
            );
            let branch = parse_ki2_str(&format!("{MOVES}{pad}変化：2手\n▲２六歩\n"))
                .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
            assert!(
                branch.moves[2].forks.is_some(),
                "{pad:?}: the branch reads as the main line carrying on: {:?}",
                branch.moves
            );
            let run = parse_ki2_str(&format!("手合割：平手\n{pad}▲７六歩 △３四歩\n"))
                .unwrap_or_else(|e| panic!("{pad:?}: {e}"));
            assert_eq!(2, run.moves.len() - 1, "{pad:?}");
        }
    }

    // D18: `まで<N>手で<結末>` and `変化：N手` are where a note goes, and a note
    // naming the move it is about opens with one. Reading a single move as the
    // line below refuses records that read as `.kif`; reading a run that fills
    // the line as a note lets a lost newline through in silence. Both directions
    // in one table.
    #[test]
    fn a_note_after_an_outcome_or_a_branch_header_is_a_note() {
        use crate::parser::parse_ki2_str;
        const MOVES: &str = "手合割：平手\n▲７六歩 △３四歩\n";
        for line in [
            "まで2手で投了 △８四歩が最善だった\n",
            "まで2手で投了　△３三桂が敗着\n",
            "まで2手で投了 惜しい将棋だった\n",
        ] {
            let jkf = parse_ki2_str(&format!("{MOVES}{line}"))
                .unwrap_or_else(|e| panic!("{line:?}: {e}"));
            assert!(
                jkf.moves.last().expect("a node").special.is_some(),
                "{line:?}: refused, or the outcome is gone"
            );
        }
        let jkf = parse_ki2_str("手合割：平手\n▲７六歩 △３四歩\n変化：2手 △８四歩から\n△８四歩\n")
            .expect("a branch header takes a note too");
        assert!(jkf.moves[2].forks.is_some(), "{:?}", jkf.moves);
        // And what fills the line is the line below, whose newline is gone.
        for line in [
            "まで1手で中断 △３四歩\n",
            "まで2手で投了 ▲７六歩 △３四歩\n",
            "まで2手で投了 変化：2手\n△８四歩\n",
            "まで2手で投了 *感想\n",
        ] {
            assert!(
                parse_ki2_str(&format!("{MOVES}{line}")).is_err(),
                "{line:?} was read as a note, and what it held is gone"
            );
        }
    }

    // A move reads the comments under it as its own, so the last mark in a
    // value need not be a move start: a swallowed `*` line puts marks behind
    // the run. Asking only the mark before the last one answers no there, and
    // the record comes back `Ok` with the game inside `header["手合割"]`.
    #[test]
    fn a_swallowed_line_is_found_behind_the_comment_it_swallowed() {
        use crate::parser::parse_ki2_str;
        for value in [
            "手合割：平手 ▲７六歩 △３四歩",
            "手合割：平手 ▲７六歩 △３四歩 *よい将棋",
            "手合割：平手 ▲７六歩 △３四歩 *▲２六歩は疑問",
            "手合割：平手 ▲７六歩 △３四歩 &▲２六歩",
        ] {
            assert!(
                parse_ki2_str(&format!("{value}\n▲７六歩 △３四歩\n")).is_err(),
                "{value:?}: the game went into the header without a word"
            );
        }
        // And the values that only quote moves still read (R-KIF-004).
        for value in [
            "消費時間：104▲379△380",
            "note：序盤は▲７六歩 △３四歩 の出だし",
            "戦型：▲居飛車",
        ] {
            let jkf = parse_ki2_str(&format!("{value}\n▲７六歩 △３四歩\n"))
                .unwrap_or_else(|e| panic!("{value:?}: {e}"));
            assert_eq!(2, jkf.moves.len() - 1, "{value:?}");
        }
    }

    // The value this check exists for is a whole game on one line, and the
    // consumer meets it inside a scan of a whole directory at start-up
    // (R-REQ-002). Asking the question once per mark made that 19.6 s for 41 KB,
    // and the record still came back `Ok` — nothing in a log to explain the
    // pause. The bound is loose on purpose: it is here to catch a quadratic
    // walk, not to measure the machine.
    #[test]
    fn a_header_value_holding_a_whole_game_is_read_in_one_pass() {
        use crate::parser::parse_ki2_str;
        let mut src = String::from("手合割：平手 ");
        for i in 0..3200 {
            src.push_str(if i % 2 == 0 {
                "▲７六歩 "
            } else {
                "△３四歩 "
            });
        }
        src.push_str("まで3200手で投了\n");
        let started = std::time::Instant::now();
        let read = parse_ki2_str(&src);
        let took = started.elapsed();
        assert!(read.is_ok(), "{:?}", read.err());
        assert!(took < std::time::Duration::from_secs(1), "{took:?}");
    }

    // D10: KIF and KI2 read the same content, so they are tolerant of the same
    // things — a closing remark after the record, a `#` line between moves.
    //
    // The tolerance stops where moves start. KIF puts every move at the head of
    // a numbered line, so skipping a line that does not start with a digit
    // throws nothing away. KI2 writes moves anywhere along a line, so a line
    // holding `▲` or `△` is never skipped: swallowing it would drop those moves
    // without a word, which is the silent loss D1 exists to remove.
    #[test]
    fn ki2_skips_the_same_lines_kif_does_and_no_more() {
        use crate::parser::parse_ki2_str;
        for (src, moves) in [
            ("手合割：平手\n▲７六歩 △３四歩\n感想：良い将棋\n", 2),
            (
                "手合割：平手\n▲７六歩 △３四歩\nまで2手で中断\n感想：良い\n",
                3,
            ),
            ("手合割：平手\n▲７六歩\n# メモ\n△３四歩\n", 2),
            ("手合割：平手\n▲７六歩 △３四歩\n解説A\n解説B\n", 2),
            // `変化：` with no number is a sentence opening with two
            // characters, which is what KIF makes of it too (D18).
            ("手合割：平手\n▲７六歩\n変化：ここから\n△３四歩\n", 2),
        ] {
            let jkf = parse_ki2_str(src).unwrap_or_else(|e| panic!("{src:?}: {e}"));
            assert_eq!(moves, jkf.moves.len() - 1, "{src:?}");
        }

        // A skipped line must not be allowed to carry a move away with it.
        for src in [
            "手合割：平手\n▲７六歩 ほげ △３四歩\n",
            "手合割：平手\n▲７六歩\nほげ △３四歩\n",
        ] {
            assert!(
                parse_ki2_str(src).is_err(),
                "{src:?} lost a move in silence"
            );
        }

        // Skipping must not swallow a `変化：` block that follows it.
        let jkf = parse_ki2_str("手合割：平手\n▲７六歩 △３四歩\n解説A\n\n変化：2手\n△８四歩\n")
            .expect("parses");
        assert!(
            jkf.moves[2].forks.is_some(),
            "the branch after the prose is gone: {:?}",
            jkf.moves
        );
    }

    #[test]
    fn parse_empty() {
        assert_eq!(
            Ok((
                "",
                (
                    JsonKifuFormat {
                        header: HashMap::new(),
                        initial: Some(Initial {
                            preset: Preset::PresetHirate,
                            data: None,
                        }),
                        moves: vec![MoveFormat::default()],
                    },
                    // Nothing was read, so no header line was consumed either.
                    false
                )
            )),
            parse("")
        );
    }

    #[test]
    fn parse_single_move() {
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 0, y: 0 },
                        piece: Kind::FU,
                        same: Some(true),
                        promote: None,
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            )),
            single_move("△同　歩")
        );
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 4, y: 7 },
                        piece: Kind::GI,
                        same: None,
                        promote: Some(false),
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            )),
            single_move("△４七銀不成")
        );
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 9, y: 9 },
                        piece: Kind::KA,
                        same: None,
                        promote: Some(true),
                        capture: None,
                        relative: None,
                    }),
                    ..Default::default()
                }
            )),
            single_move("△９九角成")
        );
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 8, y: 2 },
                        piece: Kind::KI,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: Some(Relative::U),
                    }),
                    ..Default::default()
                }
            )),
            single_move("▲８二金上")
        );
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: Some(ORIGIN_UNSTATED),
                        to: PlaceFormat { x: 8, y: 2 },
                        piece: Kind::KI,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: Some(Relative::M),
                    }),
                    ..Default::default()
                }
            )),
            single_move("▲８二金寄")
        );
        assert_eq!(
            Ok((
                "",
                MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::Black,
                        from: None,
                        to: PlaceFormat { x: 8, y: 2 },
                        piece: Kind::KI,
                        same: None,
                        promote: None,
                        capture: None,
                        relative: Some(Relative::H),
                    }),
                    ..Default::default()
                }
            )),
            single_move("▲８二金打")
        );
    }

    #[test]
    fn parse_moves() {
        assert_eq!(
            Ok((
                "",
                vec![MoveFormat {
                    comments: Some(vec![String::from("comment")]),
                    ..Default::default()
                }]
            )),
            moves(Color::Black, "*comment\n")
        );
        assert_eq!(
            Ok((
                "",
                vec![
                    MoveFormat::default(),
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(ORIGIN_UNSTATED),
                            to: PlaceFormat { x: 6, y: 8 },
                            piece: Kind::GI,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        ..Default::default()
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::White,
                            from: Some(ORIGIN_UNSTATED),
                            to: PlaceFormat { x: 3, y: 4 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        ..Default::default()
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(ORIGIN_UNSTATED),
                            to: PlaceFormat { x: 5, y: 6 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        ..Default::default()
                    }
                ]
            )),
            moves(Color::Black, "▲６八銀 △３四歩 ▲５六歩")
        )
    }

    #[test]
    fn parse_moves_with_comments() {
        assert_eq!(
            Ok((
                "",
                vec![
                    MoveFormat::default(),
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::White,
                            from: Some(ORIGIN_UNSTATED),
                            to: PlaceFormat { x: 7, y: 4 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        comments: Some(vec![
                            String::from("-2732"),
                            String::from("△３二銀(31)▲７六歩(77)△８四歩(83)"),
                        ]),
                        ..Default::default()
                    },
                    MoveFormat {
                        move_: Some(MoveMoveFormat {
                            color: Color::Black,
                            from: Some(ORIGIN_UNSTATED),
                            to: PlaceFormat { x: 7, y: 6 },
                            piece: Kind::FU,
                            same: None,
                            promote: None,
                            capture: None,
                            relative: None,
                        }),
                        comments: Some(vec![
                            String::from("2733"),
                            String::from("▲７六歩(77)△６二銀(71)▲２六歩(27)"),
                        ]),
                        ..Default::default()
                    },
                ]
            )),
            moves(
                Color::Black,
                &r#"
△７四歩
*-2732
*△３二銀(31)▲７六歩(77)△８四歩(83)
▲７六歩
*2733
*▲７六歩(77)△６二銀(71)▲２六歩(27)
"#[1..]
            )
        );
    }
}
