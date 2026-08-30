use super::kakinoki::{move_comment_line, move_to, not_move_line, parse_without_moves, piece_kind};
use crate::jkf::*;
use nom::branch::alt;
use nom::bytes::complete::tag;
use nom::character::complete::{digit1, line_ending, not_line_ending, space0};
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
        map(
            delimited(tag("("), map_res(digit1, str::parse), tag(")")),
            |d: u8| {
                Some(PlaceFormat {
                    x: d / 10,
                    y: d % 10,
                })
            },
        ),
    ))(input)
}

/// The KIF outcome words, longest first so that a word is never cut short by a
/// prefix of itself. [`MoveSpecial::from_kif_word`] holds the mapping.
const KIF_SPECIAL_WORDS: [&str; 10] = [
    "切れ負け",
    "入玉勝ち",
    "反則負け",
    "反則勝ち",
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
                    promote: promote.map(|_| true),
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
/// Reads one `<ply> <move>` line. `start` is whose turn ply 1 is.
fn move_line(start: Color, input: &str) -> IResult<&str, (usize, MoveFormat), VerboseError<&str>> {
    // The ply number has to be read before the rest: it decides whose turn it
    // is, and 反則勝ち means the *other* player committed the foul.
    let (input, i) = preceded(space0, map_res(digit1, str::parse::<usize>))(input)?;
    let side_to_move = crate::handicap::side_to_move_at_ply(start, i);
    let (input, mut mf) = preceded(space0, alt((move_special(side_to_move), move_move)))(input)?;
    let (input, time) = preceded(space0, opt(move_time))(input)?;
    let (input, _) = preceded(not_line_ending, line_ending)(input)?;
    if let Some(mmf) = &mut mf.move_ {
        mmf.color = side_to_move;
    }
    mf.time = time;
    Ok((input, (i, mf)))
}

fn move_with_comments(
    start: Color,
    input: &str,
) -> IResult<&str, (usize, MoveFormat), VerboseError<&str>> {
    let (input, (i, mf)) = move_line(start, input)?;
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

fn moves_with_index(
    start: Color,
    input: &str,
) -> IResult<&str, (usize, Vec<MoveFormat>), VerboseError<&str>> {
    let (mut input, (first_ply, first)) = move_with_comments(start, input)?;
    let mut out = vec![first];
    loop {
        // `many1` stops on `Error` and throws anything else back. Swallowing a
        // `Failure` here would drop the rest of the record without a word,
        // which is the failure this branch exists to remove.
        match move_with_comments(start, input) {
            Ok((rest, (_, mf))) => {
                out.push(mf);
                input = rest;
            }
            Err(nom::Err::Error(_)) => break,
            Err(err) => return Err(err),
        }
    }
    let (input, _) = opt(not_move_line)(input)?;
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
    fn merge_forks(
        (mut moves, mut forks): (Vec<MoveFormat>, Vec<(usize, Vec<MoveFormat>)>),
    ) -> Vec<MoveFormat> {
        let mut stack = Vec::new();
        while let Some(fork) = forks.pop() {
            stack.push(fork);
            if let Some((i, last)) = forks.last_mut() {
                while stack.last().is_some_and(|(j, _)| j > i) {
                    if let Some((j, fork)) = stack.pop() {
                        // Defend against malformed `変化:` indices (j < i, or out of range).
                        if let Some(node) = j.checked_sub(*i).and_then(|k| last.get_mut(k)) {
                            if let Some(v) = &mut node.forks {
                                v.push(fork);
                            } else {
                                node.forks = Some(vec![fork]);
                            }
                        }
                    }
                }
            }
        }
        while let Some((i, fork)) = stack.pop() {
            if i < moves.len() {
                if let Some(v) = &mut moves[i].forks {
                    v.push(fork);
                } else {
                    moves[i].forks = Some(vec![fork]);
                }
            }
        }
        moves
    }

    let (input, _) = many0(not_move_line)(input)?;
    let (mut input, main) = main_moves(start, input)?;
    let mut forks = Vec::new();
    loop {
        let (rest, _) = many0(not_move_line)(input)?;
        match moves_with_index(start, rest) {
            Ok((rest, run)) => {
                forks.push(run);
                input = rest;
            }
            Err(nom::Err::Error(_)) => break,
            Err(err) => return Err(err),
        }
    }
    Ok((input, merge_forks((main, forks))))
}

pub(crate) fn parse(input: &str) -> IResult<&str, JsonKifuFormat, VerboseError<&str>> {
    let (input, mut jkf) = parse_without_moves(input)?;
    // The side has to come from the starting position, not the ply parity: a
    // handicap record has White at every odd ply (R-HC-001).
    let start = crate::handicap::starting_side(jkf.initial.as_ref());
    let (input, moves) = entire_moves(start, input)?;
    jkf.moves.extend(moves);
    Ok((input, jkf))
}

#[cfg(test)]
mod tests {
    use super::*;

    // R-KIF-007. Every word KIF defines for an outcome, and what it means here.
    // 不戦勝 / 不戦敗 are the two the JKF vocabulary cannot express.
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
            ("不戦勝", None),
            ("不戦敗", None),
        ] {
            let line = format!("   2 {word}\n");
            let got = move_line(Color::Black, &line)
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
            let got = move_line(Color::Black, &line)
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
                        promote: None,
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
        assert!(move_line(Color::Black, "").is_err());
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
                            promote: None,
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
            move_line(Color::Black, "1 ７六歩(77) ( 0:16/00:00:16)\n")
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
            move_line(Color::Black, "3 中断 ( 0:03/ 0:00:19)\n")
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
                            promote: None,
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
            move_line(Color::Black, "   1 ７八金(69)    (00:01 / 00:00:01)\n")
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
                            promote: None,
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
                            promote: None,
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
                            promote: None,
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

    #[test]
    fn parse_entire_moves_malformed_fork_index() {
        // 変化:5 sits inside 変化:2 with offset 3, but 変化:2 only has 1 move.
        // The pre-`checked_sub` code panicked with `index out of bounds`.
        let input = &r#"
手数----指手---------消費時間--
   1 ７六歩(77)    (00:00 / 00:00:00)
   2 ８四歩(83)    (00:00 / 00:00:00)

変化：2
   2 ８六歩(83)    (00:00 / 00:00:00)

変化：5
   5 投了 ( 0:00/ 0:00:00)
"#[1..];
        let ret = entire_moves(Color::Black, input);
        let (_, moves) = ret.expect("entire_moves should not panic on malformed input");
        // 変化:2 attaches at index 2; 変化:5 is silently dropped.
        let forks = moves[2]
            .forks
            .as_ref()
            .expect("変化:2 should attach at index 2");
        assert_eq!(1, forks.len());
        assert_eq!(1, forks[0].len(), "the inner 変化:5 must NOT be merged in");
    }
}
