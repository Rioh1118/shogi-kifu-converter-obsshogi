use super::kakinoki::{move_comment_line, move_to, parse_without_moves, piece_kind};
use crate::jkf::*;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{digit1, line_ending, not_line_ending, space0};
use nom::combinator::{map, map_res, opt, value};
use nom::error::{ParseError, VerboseError};
use nom::multi::{many0, many1};
use nom::sequence::{delimited, preceded, terminated, tuple};
use nom::IResult;

fn single_move(input: &str) -> IResult<&str, MoveFormat, VerboseError<&str>> {
    map(
        tuple((
            alt((value(Color::Black, tag("▲")), value(Color::White, tag("△")))),
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
                value(Relative::U, tag("上")),
                value(Relative::M, tag("寄")),
                value(Relative::D, tag("引")),
                value(Relative::H, tag("打")),
            ))),
            // R-NOT-005: 不成 is written out, unlike KIF (R-KIF-006). The order
            // of these two does not matter — `tag` anchors at the start of the
            // input and `不成` does not begin with `成` — but keeping the longer
            // spelling first matches how the rest of this file reads.
            opt(alt((value(false, tag("不成")), value(true, tag("成"))))),
            preceded(
                tuple((space0, opt(line_ending))),
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
        let (input, phrase): (&str, &str) = terminated(not_line_ending, opt(line_ending))(input)?;
        // `まで` may be followed by `<N>手で`; the ply is already known from the
        // moves that were read, so the number is not needed.
        let phrase = phrase
            .split_once("手で")
            .map_or(phrase, |(_, rest)| rest)
            .trim();
        let side_to_move = crate::handicap::side_to_move_at_ply(start, ply);
        match MoveSpecial::from_ki2_phrase(phrase, side_to_move) {
            Some(special) => Ok((
                input,
                MoveFormat {
                    special: Some(special),
                    ..Default::default()
                },
            )),
            // `まで` matched, so this line *is* the outcome line whatever it
            // says. Reporting a recoverable error would leave the line
            // unconsumed, which ends the move list and drops the `変化：`
            // blocks after it without a word (D1: a record we cannot read has
            // to say so). `Failure` is what `opt` does not swallow.
            None => Err(nom::Err::Failure(VerboseError::from_error_kind(
                line,
                nom::error::ErrorKind::Tag,
            ))),
        }
    }
}

/// Reads a `変化：N手` header and returns `N`.
fn branch_header(input: &str) -> IResult<&str, usize, VerboseError<&str>> {
    delimited(
        preceded(many0(line_ending), tag("変化：")),
        map_res(digit1, str::parse::<usize>),
        preceded(not_line_ending, opt(line_ending)),
    )(input)
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
            // R-KI2-002: blank lines sit between runs of moves and a line can
            // start with a space — the spelling in the specification's own
            // example. Stopping at one drops the rest of the file.
            let (rest, _) = many0(alt((line_ending, tag(" "), tag("　"))))(input)?;
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
            let (rest, end) = opt(end_of_game_line(start, first_ply + out.len()))(rest)?;
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
    let (input, comments) = preceded(many0(line_ending), opt(many1(move_comment_line)))(input)?;
    let (mut input, main) = move_run(start, 1)(input)?;
    let mut out = vec![MoveFormat {
        comments,
        ..Default::default()
    }];
    out.extend(main);
    // `変化：N手` blocks, in the order the file lists them.
    let mut path = Vec::new();
    while let Ok((rest, start_ply)) = branch_header(input) {
        let (rest, branch) = move_run(start, start_ply)(rest)?;
        if branch.is_empty() {
            input = rest;
            continue;
        }
        attach_branch(&mut out[1..], &mut path, start_ply, branch);
        input = rest;
    }
    Ok((input, out))
}

pub(crate) fn parse(input: &str) -> IResult<&str, JsonKifuFormat, VerboseError<&str>> {
    let (input, mut jkf) = parse_without_moves(input)?;
    // The side has to come from the starting position, not the ply parity: a
    // handicap record has White at every odd ply (R-HC-001).
    let start = crate::handicap::starting_side(jkf.initial.as_ref());
    let (input, moves) = moves(start, input)?;
    jkf.moves.extend(moves);
    Ok((input, jkf))
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

    #[test]
    fn parse_empty() {
        assert_eq!(
            Ok((
                "",
                JsonKifuFormat {
                    header: HashMap::new(),
                    initial: Some(Initial {
                        preset: Preset::PresetHirate,
                        data: None,
                    }),
                    moves: vec![MoveFormat::default()],
                }
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
