//! Reads CSA (R-CSA-001 to R-CSA-009), V2.2 and V3.0.
//!
//! A line holds statements separated by `,` (R-CSA-009). A statement that
//! opens with `'` is a comment and runs to the end of its line, commas and all.
//! The rest come in a fixed order: the version, the players and the `$`
//! keywords, the initial position, the side to move, and then moves, special
//! moves and times.
//!
//! Every statement this reader does not know is an error that names its line.
//! A reader that skips what it cannot read returns a shorter record as if it
//! were the whole one (D1).

use crate::error::ParseError;
use crate::handicap::HIRATE_BOARD;
use crate::jkf::*;
use std::collections::HashMap;

/// Reads `text`, which has one line ending (`\n`) and no byte-order mark.
///
/// The record comes back as read: the initial position is a board
/// ([`Preset::PresetOther`]), `promote` is not stated and `from` of a drop is
/// absent. Normalizing it is the caller's.
pub(super) fn parse(text: &str) -> Result<JsonKifuFormat, ParseError> {
    let mut reader = Reader::default();
    for (index, line) in text.split('\n').enumerate() {
        // A line of nothing but blanks is no statement (GAP-040).
        if line.trim_matches([' ', '\t']).is_empty() {
            continue;
        }
        let mut rest = line;
        while !rest.trim_end_matches([' ', '\t']).is_empty() {
            let (statement, after) = next_statement(rest);
            // Blanks behind a statement pad it to a column (`+7776FU  ,T0` in
            // programs of the 2000s), except in a comment, which is its text
            // as written. A rank whose last square is empty loses that
            // square's blank too, and `board_rank` reads it without one.
            let statement = if statement.starts_with('\'') {
                statement
            } else {
                statement.trim_end_matches([' ', '\t'])
            };
            reader
                .statement(statement)
                .map_err(|why| ParseError::Csa(format!("at line {}: {why}\n{line}", index + 1)))?;
            rest = after;
        }
    }
    reader.finish()
}

/// The statement at the head of `rest`, and what follows it on the line.
fn next_statement(rest: &str) -> (&str, &str) {
    if rest.starts_with('\'') {
        return (rest, "");
    }
    match rest.split_once(',') {
        Some((statement, after)) => (statement, after),
        None => (rest, ""),
    }
}

#[derive(Default)]
struct Reader {
    header: HashMap<String, String>,
    /// Comments before the first move: the initial position's (R-JKF-001).
    initial_comments: Vec<String>,
    position: Option<Position>,
    side_to_move: Option<Color>,
    moves: Vec<MoveFormat>,
    /// A `/` ended the record. What follows is another record, and JKF holds
    /// one (D31).
    ended: bool,
}

/// The initial position as the `P` statements build it.
struct Position {
    board: [[Piece; 9]; 9],
    hands: [Hand; 2],
    /// `AL` gives one side everything not yet placed (R-CSA-006).
    rest_to: Option<Color>,
}

impl Reader {
    fn statement(&mut self, statement: &str) -> Result<(), String> {
        // A file may hold several records separated by `/` (R-CSA-001's
        // section 2.1). JKF holds one, so the first is the record, as tsshogi
        // reads it (D31).
        if self.ended {
            return Ok(());
        }
        if let Some(comment) = statement.strip_prefix('\'') {
            // Above the side-to-move line a comment labels the file's layout
            // (`'対局者名`, `'開始日時`) rather than saying anything about the
            // game, and the JKF author's reader drops those; below it, a
            // comment belongs to the move above it, or to the initial position
            // before the first move (D30). The V3.0 encoding line is one of the
            // dropped: the bytes were decoded before this reader saw them.
            if self.side_to_move.is_some() {
                self.comment(comment.to_owned());
            }
            return Ok(());
        }
        if statement == "/" {
            self.ended = true;
            return Ok(());
        }
        // Programs write `$END_TIME` once the game is over, under the moves
        // (the 世界コンピュータ将棋選手権 relay does), so a keyword is a header
        // line wherever it sits.
        if let Some(keyword) = statement.strip_prefix('$') {
            let (key, value) = keyword
                .split_once(':')
                .ok_or("a `$` keyword has no `:` before its value")?;
            self.header.insert(header_key(key), value.to_owned());
            return Ok(());
        }
        if self.side_to_move.is_none() {
            return self.before_the_moves(statement);
        }
        self.in_the_moves(statement)
    }

    fn comment(&mut self, comment: String) {
        match self.moves.last_mut() {
            Some(node) => node.comments.get_or_insert_with(Vec::new).push(comment),
            None => self.initial_comments.push(comment),
        }
    }

    /// The version, the header and the position, up to the side to move.
    fn before_the_moves(&mut self, statement: &str) -> Result<(), String> {
        if is_version(statement) {
            return Ok(());
        }
        if let Some(name) = statement.strip_prefix("N+") {
            self.header.insert("先手".to_owned(), name.to_owned());
            return Ok(());
        }
        if let Some(name) = statement.strip_prefix("N-") {
            self.header.insert("後手".to_owned(), name.to_owned());
            return Ok(());
        }
        if let Some(position) = statement.strip_prefix('P') {
            return self.position_statement(position);
        }
        match statement {
            "+" => self.side_to_move = Some(Color::Black),
            "-" => self.side_to_move = Some(Color::White),
            _ => return Err("this is not a CSA statement".to_owned()),
        }
        if self.position.is_none() {
            return Err("the side to move comes before any initial position".to_owned());
        }
        Ok(())
    }

    fn position_statement(&mut self, statement: &str) -> Result<(), String> {
        let position = self.position.get_or_insert_with(|| Position {
            board: [[Piece::empty(); 9]; 9],
            hands: [Hand::default(); 2],
            rest_to: None,
        });
        if let Some(dropped) = statement.strip_prefix('I') {
            // The even game, less the pieces listed (R-CSA-006).
            position.board = HIRATE_BOARD;
            for (square, _) in squares_and_pieces(dropped)? {
                let (file, rank) = square.ok_or("`PI` lists a square on the board")?;
                position.board[file][rank] = Piece::empty();
            }
            return Ok(());
        }
        if let Some(rank) = statement.chars().next().and_then(|c| c.to_digit(10)) {
            if !(1..=9).contains(&rank) {
                return Err("a rank of the board is 1 to 9".to_owned());
            }
            return board_rank(position, rank as usize - 1, &statement[1..]);
        }
        let color = match statement.chars().next() {
            Some('+') => Color::Black,
            Some('-') => Color::White,
            _ => return Err("this is not a CSA position statement".to_owned()),
        };
        for (square, kind) in squares_and_pieces(&statement[1..])? {
            match (square, kind) {
                (None, None) => position.rest_to = Some(color),
                (None, Some(kind)) => position.hands[color as usize]
                    .increment(kind)
                    .ok_or("a king or a promoted piece cannot be held in hand")?,
                (Some((file, rank)), Some(kind)) => {
                    position.board[file][rank] = Piece {
                        color: Some(color),
                        kind: Some(kind),
                    }
                }
                (Some(_), None) => return Err("`AL` is a hand, not a square".to_owned()),
            }
        }
        Ok(())
    }

    /// Moves, special moves and times.
    ///
    /// Past a special move, a statement that is none of them ends the record
    /// rather than the reading (D36): what follows the end of a game is a
    /// second game run on without its `/`, or a note on the game, and neither
    /// is this record. Before one, it is a record cut short or broken, and
    /// that is an error naming its line (D1).
    fn in_the_moves(&mut self, statement: &str) -> Result<(), String> {
        let read = self.move_special_or_time(statement);
        if read.is_err() && self.moves.iter().any(|node| node.special.is_some()) {
            self.ended = true;
            return Ok(());
        }
        read
    }

    fn move_special_or_time(&mut self, statement: &str) -> Result<(), String> {
        if let Some(word) = statement.strip_prefix('%') {
            let special = special_move(word).ok_or("this special move is not a CSA keyword")?;
            self.moves.push(MoveFormat {
                special: Some(special),
                ..Default::default()
            });
            return Ok(());
        }
        if let Some(seconds) = statement.strip_prefix('T') {
            let node = self
                .moves
                .last_mut()
                .ok_or("a time comes before any move")?;
            node.time = Some(Time {
                now: time_spent(seconds)?,
                total: TimeFormat::default(),
            });
            return Ok(());
        }
        self.moves.push(MoveFormat {
            move_: Some(move_statement(statement)?),
            ..Default::default()
        });
        Ok(())
    }

    fn finish(self) -> Result<JsonKifuFormat, ParseError> {
        let Some(mut position) = self.position else {
            return Err(ParseError::Csa("there is no initial position".to_owned()));
        };
        let color = self
            .side_to_move
            .ok_or_else(|| ParseError::Csa("there is no side to move".to_owned()))?;
        let after_board = pieces_off_the_board(&position)?;
        if let Some(side) = position.rest_to {
            let unplaced = unplaced_pieces(after_board, &position)?;
            // In addition to what that side's own `P` lines put in its hand.
            let hand = &mut position.hands[side as usize];
            for (slot, more) in [
                (&mut hand.FU, unplaced.FU),
                (&mut hand.KY, unplaced.KY),
                (&mut hand.KE, unplaced.KE),
                (&mut hand.GI, unplaced.GI),
                (&mut hand.KI, unplaced.KI),
                (&mut hand.KA, unplaced.KA),
                (&mut hand.HI, unplaced.HI),
            ] {
                *slot += more;
            }
        }
        let mut moves = vec![MoveFormat {
            comments: (!self.initial_comments.is_empty()).then_some(self.initial_comments),
            ..Default::default()
        }];
        moves.extend(self.moves);
        Ok(JsonKifuFormat {
            header: self.header,
            initial: Some(Initial {
                preset: Preset::PresetOther,
                data: Some(StateFormat {
                    color,
                    board: position.board,
                    hands: position.hands,
                }),
            }),
            moves,
        })
    }
}

/// `V2`, `V2.1`, `V2.2` or `V3.0`. Without one the record is the 1997 format,
/// which this reader reads the same way.
fn is_version(statement: &str) -> bool {
    statement
        .strip_prefix('V')
        .is_some_and(|v| !v.is_empty() && v.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

/// The header key a `$` keyword is filed under: the KIF name where JKF has
/// one (R-CSA-004), and the keyword itself otherwise, so that nothing the file
/// says is dropped. The value is kept as written.
fn header_key(keyword: &str) -> String {
    match keyword {
        "EVENT" => "棋戦",
        "SITE" => "場所",
        "START_TIME" => "開始日時",
        "END_TIME" => "終了日時",
        "TIME_LIMIT" => "持ち時間",
        "OPENING" => "戦型",
        other => other,
    }
    .to_owned()
}

/// A rank of the board in the bulk form: nine three-character cells, `+FU`,
/// `-KY` or ` * ` for an empty square (R-CSA-006).
fn board_rank(position: &mut Position, rank: usize, cells: &str) -> Result<(), String> {
    // Nine squares of three characters (R-CSA-006): `+FU`, `-KY`, or ` * ` for
    // an empty one. Mail and web pages collapse the blanks of a run of empty
    // squares (` *  * ` arrives as ` * * `), and tsshogi reads that shape, so
    // an empty square is a `*` with at most one blank on either side. Fewer
    // than nine squares is pieces missing, not empty squares (D1).
    let mut rest = cells;
    for file_from_left in 0..9 {
        let piece = match rest.chars().next() {
            Some(sign @ ('+' | '-')) => {
                let code = rest.get(1..3).ok_or("this square holds no CSA piece")?;
                let kind = piece_kind(code).ok_or("this square holds no CSA piece")?;
                rest = &rest[3..];
                Piece {
                    color: Some(if sign == '+' {
                        Color::Black
                    } else {
                        Color::White
                    }),
                    kind: Some(kind),
                }
            }
            _ => {
                let after = rest.strip_prefix(' ').unwrap_or(rest);
                let after = after
                    .strip_prefix('*')
                    .ok_or("a rank of the board has fewer than nine squares")?;
                rest = after.strip_prefix(' ').unwrap_or(after);
                Piece::empty()
            }
        };
        position.board[8 - file_from_left][rank] = piece;
    }
    if !rest.trim_matches([' ', '\t']).is_empty() {
        return Err("a rank of the board has more than nine squares".to_owned());
    }
    Ok(())
}

/// `<square><piece>` pairs, as `PI` and `P+` list them: `82HI22KA`. The
/// square is `None` for `00`, the hand, and the piece `None` for `AL`.
#[allow(clippy::type_complexity)]
fn squares_and_pieces(list: &str) -> Result<Vec<(Option<(usize, usize)>, Option<Kind>)>, String> {
    if !list.len().is_multiple_of(4) || !list.is_ascii() {
        return Err("a position list is pairs of two digits and two letters".to_owned());
    }
    (0..list.len())
        .step_by(4)
        .map(|at| {
            let square = square(&list[at..at + 2])?;
            let piece = &list[at + 2..at + 4];
            let kind = match piece {
                "AL" if square.is_none() => None,
                _ => Some(piece_kind(piece).ok_or("this is not a CSA piece")?),
            };
            Ok((square, kind))
        })
        .collect()
}

/// A two-digit square, as `(file - 1, rank - 1)`, or `None` for `00`.
/// Positions are 1-9 in each direction (R-CSA-005).
fn square(digits: &str) -> Result<Option<(usize, usize)>, String> {
    let mut chars = digits.chars().map(|c| c.to_digit(10));
    match (chars.next().flatten(), chars.next().flatten()) {
        (Some(0), Some(0)) => Ok(None),
        (Some(file @ 1..=9), Some(rank @ 1..=9)) => {
            Ok(Some((file as usize - 1, rank as usize - 1)))
        }
        _ => Err("a square is two digits 1 to 9, or 00 for the hand".to_owned()),
    }
}

/// `+7776FU`: the side, the origin (`00` for a drop), the destination, and the
/// piece *after* the move (R-CSA-007).
fn move_statement(statement: &str) -> Result<MoveMoveFormat, String> {
    if statement.len() != 7 || !statement.is_ascii() {
        return Err("a move is a sign, four digits and a piece: `+7776FU`".to_owned());
    }
    let color = match &statement[..1] {
        "+" => Color::Black,
        "-" => Color::White,
        _ => return Err("a move opens with + or -".to_owned()),
    };
    let from = square(&statement[1..3])?;
    let to = square(&statement[3..5])?.ok_or("a move cannot go to the hand")?;
    let piece = piece_kind(&statement[5..]).ok_or("this is not a CSA piece")?;
    let place = |(file, rank): (usize, usize)| PlaceFormat {
        x: file as u8 + 1,
        y: rank as u8 + 1,
    };
    Ok(MoveMoveFormat {
        color,
        // JKF says a move is a drop by leaving `from` out (R-JKF-003).
        from: from.map(place),
        to: place(to),
        piece,
        same: None,
        promote: None,
        capture: None,
        relative: None,
    })
}

/// The seconds a `T` statement gives the move above it (R-CSA-007). V3.0 adds
/// up to three decimal places, which JKF has no field for.
fn time_spent(seconds: &str) -> Result<TimeFormat, String> {
    let whole = seconds.split_once('.').map_or(seconds, |(whole, _)| whole);
    let s: u64 = whole
        .parse()
        .map_err(|_| "a time is a number of seconds".to_owned())?;
    let h = s / 3600;
    Ok(TimeFormat {
        // Saturating, not wrapping: a time that long is broken input either
        // way, and a wrapped one reads as a short, ordinary move.
        h: (h > 0).then(|| h.min(u64::from(u16::MAX)) as u16),
        m: ((s / 60) % 60) as u16,
        s: (s % 60) as u16,
    })
}

fn piece_kind(code: &str) -> Option<Kind> {
    Some(match code {
        "FU" => Kind::FU,
        "KY" => Kind::KY,
        "KE" => Kind::KE,
        "GI" => Kind::GI,
        "KI" => Kind::KI,
        "KA" => Kind::KA,
        "HI" => Kind::HI,
        "OU" => Kind::OU,
        "TO" => Kind::TO,
        "NY" => Kind::NY,
        "NK" => Kind::NK,
        "NG" => Kind::NG,
        "UM" => Kind::UM,
        "RY" => Kind::RY,
        _ => return None,
    })
}

/// The `%` keywords of V2.2 and V3.0 (R-CSA-007).
fn special_move(word: &str) -> Option<MoveSpecial> {
    Some(match word {
        "TORYO" => MoveSpecial::SpecialToryo,
        "CHUDAN" => MoveSpecial::SpecialChudan,
        "SENNICHITE" => MoveSpecial::SpecialSennichite,
        "TIME_UP" => MoveSpecial::SpecialTimeUp,
        "ILLEGAL_MOVE" => MoveSpecial::SpecialIllegalMove,
        "+ILLEGAL_ACTION" => MoveSpecial::SpecialIllegalActionBlack,
        "-ILLEGAL_ACTION" => MoveSpecial::SpecialIllegalActionWhite,
        "JISHOGI" => MoveSpecial::SpecialJishogi,
        "KACHI" => MoveSpecial::SpecialKachi,
        "HIKIWAKE" => MoveSpecial::SpecialHikiwake,
        "MATTA" => MoveSpecial::SpecialMatta,
        "TSUMI" => MoveSpecial::SpecialTsumi,
        "FUZUMI" => MoveSpecial::SpecialFuzumi,
        "ERROR" => MoveSpecial::SpecialError,
        "MAX_MOVES" => MoveSpecial::SpecialMaxMoves,
        _ => return None,
    })
}

/// The pieces of one set that are not on the board. A board holding more of a
/// piece than a set contains is broken input, not a negative count.
fn pieces_off_the_board(position: &Position) -> Result<Hand, ParseError> {
    let mut all = Hand {
        FU: 18,
        KY: 4,
        KE: 4,
        GI: 4,
        KI: 4,
        KA: 2,
        HI: 2,
    };
    for kind in position
        .board
        .iter()
        .flatten()
        .filter_map(|piece| piece.kind.map(Kind::unpromoted))
        .filter(|kind| *kind != Kind::OU)
    {
        all.decrement(kind)
            .ok_or(ParseError::CsaConvert("too many pieces on the board"))?;
    }
    Ok(all)
}

/// What `AL` hands over: the pieces neither on the board nor already in a
/// hand. Only counted when `AL` asks; a hand beyond a set is otherwise read as
/// written, the same as the writer writes it.
fn unplaced_pieces(mut all: Hand, position: &Position) -> Result<Hand, ParseError> {
    for hand in &position.hands {
        for (kind, count) in [
            (Kind::FU, hand.FU),
            (Kind::KY, hand.KY),
            (Kind::KE, hand.KE),
            (Kind::GI, hand.GI),
            (Kind::KI, hand.KI),
            (Kind::KA, hand.KA),
            (Kind::HI, hand.HI),
        ] {
            for _ in 0..count {
                all.decrement(kind)
                    .ok_or(ParseError::CsaConvert("too many pieces in hand"))?;
            }
        }
    }
    Ok(all)
}

#[cfg(test)]
mod tests;
