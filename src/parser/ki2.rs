use super::kakinoki::{
    broken_line, comments_on_the_initial_position, ends_here, ends_word, fork_header_ply,
    is_reportable_empty_block, move_comment_line, move_to, not_move_line, opens_shared_line,
    parse_without_moves, piece_kind, program_comment_line, LineShapes, WhereABoardCouldBe,
    SIDE_MARKS,
};
use crate::jkf::*;
use crate::notation::is_padding;
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

/// The normal spelling of what a move says about which piece made it
/// (R-NOT-004).
///
/// Reads the table the writer writes from (`notation::relative_word`), longest
/// first so that `左上` is not taken as `左` with `上` left over. A second table
/// here would only disagree with the writer's when someone reads back a file
/// this crate wrote.
fn relative_word_here(input: &str) -> IResult<&str, Relative, VerboseError<&str>> {
    for relative in Relative::ALL {
        if let Ok((rest, _)) =
            tag::<_, _, VerboseError<&str>>(crate::notation::relative_word(relative))(input)
        {
            return Ok((rest, relative));
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
                relative_word_here,
                // R-NOT-006 / R-KI2-005: 飛・角の「上」は紙面で「行」と書かれる。
                // 書き出しは正規形（`notation::relative_word`）だけを使い、
                // 読み取りでは揺れも受ける。
                value(Relative::U, tag("行")),
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
    carries_a_line: is_header_value_with_moves,
    opens_line: opens_ki2_line,
};

/// What a KI2 line looks like: the shapes both formats have, or a run of moves.
fn opens_ki2_line(head: &str) -> bool {
    opens_shared_line(head) || is_whole_line_move_run(head)
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
/// So the same line [`is_header_value_with_moves`] draws: what a lost newline
/// leaves behind is the line below it, whole, and a line of KI2 is moves all the
/// way to its end. A note stops being one.
fn is_whole_line_move_run(head: &str) -> bool {
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
/// stops. A KI2 record whose initial position lost its newline puts the whole
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
fn is_header_value_with_moves(value: &str) -> bool {
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

/// Reads the `まで<N>手で…` line that KI2 uses instead of a special move line.
///
/// `ply` is the ply the special move occupies, which decides whose turn it is and
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
        // `まで` may be followed by `<N>手で`, or by `<N>手` alone in the
        // tsume spelling `まで61手詰` (GAP-034); the ply is already known from
        // the moves that were read, so the number is not needed.
        let after_number =
            input.trim_start_matches(|c: char| c.is_ascii_digit() || ('０'..='９').contains(&c));
        let phrase = match after_number.strip_prefix('手') {
            Some(rest) if after_number.len() < input.len() => {
                rest.strip_prefix('で').unwrap_or(rest)
            }
            _ => input,
        };
        let phrase = phrase.trim_start_matches(is_padding);
        // The special move word owns the line up to the first space, note marker or
        // line ending. This line is the only place KI2 has to put a special move
        // (R-KI2-006, D5), and it is also where a note about the game goes
        // (`まで2手で中断 （▲有利）`, D18) — so the word ends where the note
        // begins. What opens a note is D17's table, the same one the line-end
        // rule uses, and what is left goes to that rule below.
        let end = phrase.find(ends_word).unwrap_or(phrase.len());
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
            // `まで` matched, so this line *is* the special move line whatever it
            // says. Reporting a recoverable error would leave the line
            // unconsumed, which ends the move list and drops the `変化：`
            // blocks after it without a word (D1: a record we cannot read has
            // to say so). `Failure` is what `opt` does not swallow.
            None => Err(broken_line(
                line,
                "this special move is not one of the words KI2 has",
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
/// dropping what was there takes the first move of the fork with it whenever
/// the newline between them is lost.
fn fork_header(input: &str) -> IResult<&str, (usize, &str), VerboseError<&str>> {
    let (line, _) = many0(line_ending)(input)?;
    let (rest, ply) = fork_header_ply(line)?;
    // KI2 is the reader that uses the number (D3, D19), so it is the one that
    // says when the number cannot be used. Plies count from 1 (R-JKF-001), so
    // `変化：０手` names no move for a fork to be an alternative to; carried
    // on, it reaches `attach_fork`, which cannot find a departure point and
    // returns — taking the fork and everything under it without a word
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

/// Reads one run of moves — the main line, or one `変化：` block — with the
/// special move lines among them.
///
/// A special move does not end the run. A game that was interrupted and resumed
/// records `中断` in the middle and keeps going, and stopping at the first
/// `まで…` line drops every move after it without saying so.
///
/// `where_a_board_could_be` is what the run starts with, and the run narrows its
/// own copy as it reads (`WhereABoardCouldBe::after`): a board diagram can still
/// be arriving above the first move of the main line, and the skip below must
/// leave its `|` and `+` lines alone (GAP-007). What counts as "a move was read"
/// is that method's to answer, not this function's — the caller asks it again of
/// the run this returns.
fn move_run(
    start: Color,
    first_ply: usize,
    where_a_board_could_be: WhereABoardCouldBe,
) -> impl FnMut(&str) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    move |mut input| {
        let mut out = Vec::new();
        // Only a move puts the opening block behind us. A comment or a `まで…`
        // line can sit above a board — the last game's special move left at the head
        // of the file does not mean the diagram below it is not one (GAP-007).
        let mut where_a_board_could_be = where_a_board_could_be;
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
                    is_prose_only_line(where_a_board_could_be, input)
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
            where_a_board_could_be = where_a_board_could_be.after(&v);
            out.extend(v);
            // The ply a special move line names counts moves and special moves, not
            // nodes: a comment-only node writes no line for anyone to number.
            // The writers count the same way (`MoveFormat::occupies_ply`).
            let numbered = out.iter().filter(|mf| mf.occupies_ply()).count();
            // Saturating because `first_ply` comes from a `変化：N手` line and a
            // record can say any `N` (`fork_header_ply`, D19). The ply is only
            // used to decide whose turn a special move is, and a fork that far out
            // has no node to attach to anyway — an overflow here would be a
            // panic in a Tauri command (R-REQ-002).
            let ply = first_ply.saturating_add(numbered);
            let (rest, end) = opt(end_of_game_line(start, ply))(rest)?;
            let read_end = end.is_some();
            // The comments under the special move are the special move's, the same as the
            // ones under a move are that move's (R-KIF-010) — `single_move`
            // reads its own the same way. Left for the next turn of this loop
            // they become a node of their own, and the record that comes back
            // from `.ki2` has a node the one from `.kif` does not (D18).
            let (rest, end) = match end {
                Some(mut end) => {
                    let (rest, comments) = opt(many1(move_comment_line))(rest)?;
                    end.comments = comments;
                    (rest, Some(end))
                }
                None => (rest, None),
            };
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
/// whoever reads it, not to this skip. A `まで…` is the special move and [`move_run`]
/// reads it; a `変化：` says a fork starts and [`moves`] reads it; `*` and `&`
/// are comments. Skipping one takes what it said with it, and the record comes
/// back `Ok` without it — a special move that never happened, or a fork whose
/// moves are read as the main line carrying on (R-JKF-004). Two skips is how
/// that happens: one of them learns a shape and the other goes on swallowing it.
///
/// Narrower than the KIF skip in one more way. KIF puts every move at the head
/// of its own numbered line, so a line that does not start with a digit holds no
/// move. KI2 writes moves anywhere along a line (R-KI2-001), so skipping a whole
/// line can throw moves away — the silent loss D1 exists to remove. A line
/// carrying `▲` or `△` is therefore never skippable, and the leftover-input
/// check reports it instead.
fn is_prose_only_line(
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, &str, VerboseError<&str>> {
    // Past the padding, the same as `begins_next_line` looks past it. A
    // `変化：` one tab in is still a `変化：`, and asking from the raw start
    // hands it to the skip below — the fork is gone and its moves carry on as
    // the main line (R-JKF-004).
    if opens_shared_line(input.trim_start_matches(is_padding)) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Not,
        )));
    }
    let (rest, line) = not_move_line(where_a_board_could_be, input)?;
    // A line that opens with a note marker is a note, whatever it quotes:
    // `※▲２六歩が本筋` holds a mark but no move, since a move never starts with
    // `※`. KIF skips it as prose, and KI2 refused the whole file over it
    // (GAP-029).
    let is_a_note = line
        .trim_start_matches(is_padding)
        .starts_with(super::kakinoki::NOTE_MARKERS);
    if !is_a_note && line.contains(|c| SIDE_MARKS.iter().any(|(mark, _)| *mark == c)) {
        return Err(nom::Err::Error(VerboseError::from_error_kind(
            input,
            nom::error::ErrorKind::Verify,
        )));
    }
    Ok((rest, line))
}

/// Attaches `fork` as an alternative to the move at ply `start_ply`.
///
/// `path` is the run of fork choices taken to get here, as
/// `(ply the fork departs from, index among that node's forks)`. KI2 names
/// the departure ply explicitly, and a later `変化：` is read against the fork
/// most recently entered, so the path is first cut back to the forks that
/// start before `start_ply`.
fn attach_fork(
    main: &mut [MoveFormat],
    path: &mut Vec<(usize, usize)>,
    start_ply: usize,
    fork: Vec<MoveFormat>,
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
    forks.push(fork);
    path.push((start_ply, forks.len() - 1));
}

/// Reads the main line and every `変化：N手` block. `start` is whose turn ply 1
/// is, which the special move line needs and the ply parity cannot supply.
fn moves(
    start: Color,
    where_a_board_could_be: WhereABoardCouldBe,
    input: &str,
) -> IResult<&str, Vec<MoveFormat>, VerboseError<&str>> {
    // Everything above the first move, in one loop.
    //
    // Comments belong to the initial position (R-KIF-010) and are kept; the
    // rest is read and dropped. A `変化：` among them names no move to be an
    // alternative to, so there is nothing to act on — KIF does the same
    // (`kif::skippable_line_above_the_main_line`), and tsshogi does it deliberately: its
    // `kakinoki.mjs` cites shogihome #570 for records whose header block holds
    // one, and says a `変化：` before the moves start must be ignored. Left in
    // place it reaches the fork loop below, which reads the whole game as a
    // fork of a main line that has no node to hang it on — `attach_fork`
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
            nom::combinator::recognize(fork_header),
            nom::combinator::recognize(program_comment_line),
            nom::combinator::recognize(|input| is_prose_only_line(where_a_board_could_be, input)),
            line_ending,
        ))(input)
        {
            Ok((rest, _)) => input = rest,
            Err(nom::Err::Failure(err)) => return Err(nom::Err::Failure(err)),
            Err(_) => break,
        }
    }
    let comments = (!comments.is_empty()).then_some(comments);
    let (mut input, main) = move_run(start, 1, where_a_board_could_be)(input)?;
    // What the main line read decides it for the skips below: a record whose
    // first move is still to come may have its diagram anywhere above it.
    let where_a_board_could_be = where_a_board_could_be.after(&main);
    let mut out = vec![MoveFormat {
        comments,
        ..Default::default()
    }];
    out.extend(main);
    // `変化：N手` blocks, in the order the file lists them.
    let mut path = Vec::new();
    loop {
        match fork_header(input) {
            Ok((mut rest, (mut start_ply, mut header))) => {
                // Headers in a row are one declaration: the last of them says
                // which ply the fork leaves, and the ones over it are spare
                // lines rather than forks that went missing.
                loop {
                    match fork_header(rest) {
                        Ok((after, (ply, line))) => {
                            rest = after;
                            start_ply = ply;
                            header = line;
                        }
                        // The same as at the top of the loop: a header that ran
                        // into the moves under it is the fork, gone. Taking it
                        // for "no more headers" reports the header above this
                        // one, and says its block is empty when what is broken
                        // is the line below it.
                        Err(err @ nom::Err::Failure(_)) => return Err(err),
                        Err(_) => break,
                    }
                }
                let (rest, fork) = move_run(start, start_ply, where_a_board_could_be)(rest)?;
                if fork.is_empty() {
                    // The header says a fork follows. Reading nothing under it
                    // means the fork is gone, and carrying on returns a record
                    // that is a whole fork short without saying so.
                    //
                    // Unless the line says more than the number. `変化：2手を参照`
                    // is a note *about* a fork — nothing was ever under it to
                    // lose — while `変化：2手` alone is a fork that went
                    // missing. Both are fork declarations (D20); what tells
                    // them apart is whether the line says anything else (D1).
                    if is_reportable_empty_block(header) {
                        return Err(broken_line(header, "a 変化 block with no moves under it"));
                    }
                    input = rest;
                    continue;
                }
                attach_fork(&mut out[1..], &mut path, start_ply, fork);
                input = rest;
                continue;
            }
            // A `Failure` is `fork_header` saying the line *is* a header and is
            // broken — it ran into the moves under it. Taking it for "not a
            // fork header" hands the line to the skip below, which drops the
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
            is_prose_only_line(where_a_board_could_be, input)
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
    let (rest, (mut jkf, header_comments, where_a_board_could_be, read_header)) =
        parse_without_moves(SHAPES, input)?;
    // The side has to come from the initial position, not the ply parity: a
    // handicap record has White at every odd ply (R-HC-001).
    let start = crate::handicap::starting_side(jkf.initial.as_ref());
    let (input, moves) = moves(start, where_a_board_could_be, rest)?;
    jkf.moves.extend(moves);
    comments_on_the_initial_position(header_comments, &mut jkf.moves);
    Ok((input, (jkf, read_header)))
}

#[cfg(test)]
mod tests;
