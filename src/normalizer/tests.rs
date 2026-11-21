use super::*;

// R-RULE-002: a kifu records what was played, illegal moves included, and
// R-KIF-006 / R-NOT-005 leave the notation as the only place a promotion is
// stated. Working `promote` out from the board instead throws this `成`
// away without a word, and the record is read — and written back — as a
// plain 7六歩 that the file it came from does not contain.
#[test]
fn a_stated_promotion_survives_a_move_the_rules_cannot_promote() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩成(77)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let mv = jkf.moves[1].move_.expect("a move");
    assert_eq!(Some(true), mv.promote, "the record says it promoted");
    assert!(
        crate::converter::ToKif::try_to_kif_owned(&jkf)
            .expect("writes")
            .contains("７六歩成(77)"),
        "and it is written back"
    );
}

// CSA has no `成`. It names the piece the move left behind (R-CSA-007), so a
// CSA promotion arrives as a `piece` that is the promoted form of the one on
// `from` and an empty `promote` — a statement in the record just the same.
#[test]
fn a_csa_states_a_promotion_by_naming_the_piece_it_became() {
    let csa = "V2.2\nPI\n+\n+7776FU\n-3334FU\n+8822UM\n";
    let jkf = crate::parser::parse_csa_str(csa).expect("parses");
    let mv = jkf.moves[3].move_.expect("a move");
    assert_eq!(Some(true), mv.promote);
    assert_eq!(
        Kind::KA,
        mv.piece,
        "D12: `piece` names the piece that moved, whatever the record spelled"
    );
}

// The piece name is how CSA states a promotion (R-CSA-007), not a second
// opinion on a record that already stated one. Read over a statement, it
// puts a `成` into a record that has none: `７五と` on a square holding a
// pawn is a spelling that disagrees with the board, and D12 answers a
// disagreeing `piece` by taking the board's — not by promoting the move.
#[test]
fn a_piece_name_that_disagrees_with_the_board_does_not_promote_the_move() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ７五と(76)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let mv = jkf.moves[3].move_.expect("a move");
    assert_eq!(None, mv.promote, "7五 is nowhere near the enemy camp");
    assert_eq!(Kind::FU, mv.piece, "D12: the board names the piece");
    let back = crate::converter::ToKif::try_to_kif_owned(&jkf).expect("writes");
    assert!(
        !back.contains('成'),
        "no 成 in the record, so none in the write-back: {back}"
    );
}

// The same the other way round, on the entry the consumer's save path feeds
// (R-REQ-002): a JKF that says `promote: false` while naming the promoted
// piece. D12 puts the record first, so the statement wins.
#[test]
fn a_stated_promote_false_is_not_overruled_by_the_piece_name() {
    let json = r#"{"header":{},"initial":{"preset":"HIRATE"},"moves":[{},
            {"move":{"color":0,"from":{"x":7,"y":7},"to":{"x":7,"y":6},"piece":"FU"}},
            {"move":{"color":1,"from":{"x":3,"y":3},"to":{"x":3,"y":4},"piece":"FU"}},
            {"move":{"color":0,"from":{"x":8,"y":8},"to":{"x":2,"y":2},"piece":"UM","promote":false}}]}"#;
    let jkf = crate::parser::parse_jkf_str(json).expect("parses");
    let mv = jkf.moves[3].move_.expect("a move");
    // R-NOT-005 keeps the `false`: 2二 is the enemy camp, so not promoting
    // there is worth recording.
    assert_eq!(Some(false), mv.promote);
    assert_eq!(Kind::KA, mv.piece);
}

// A drop has no origin (R-JKF-003), so there is no square to ask what piece
// moved or what it took, and `piece` and `capture` are left as they came.
// The contract in `normalize` is what the consumer reads before
// trusting those fields, so the exception is worth a test of its own.
//
// `same` is not among them for a reason that is not worth a case: a drop
// whose destination is the square the move before went to is a drop onto an
// occupied square, so there is no record to write it in.
#[test]
fn a_drop_keeps_the_fields_an_origin_would_have_decided() {
    let json = r#"{"header":{},"initial":{"preset":"HIRATE"},"moves":[{},
            {"move":{"color":0,"from":{"x":7,"y":7},"to":{"x":7,"y":6},"piece":"FU"}},
            {"move":{"color":1,"from":{"x":3,"y":3},"to":{"x":3,"y":4},"piece":"FU"}},
            {"move":{"color":0,"from":{"x":8,"y":8},"to":{"x":2,"y":2},"piece":"KA","promote":true}},
            {"move":{"color":1,"from":{"x":3,"y":1},"to":{"x":2,"y":2},"piece":"GI"}},
            {"move":{"color":0,"to":{"x":4,"y":5},"piece":"KA","capture":"HI"}}]}"#;
    let jkf = crate::parser::parse_jkf_str(json).expect("parses");
    let mv = jkf.moves[5].move_.expect("a move");
    assert_eq!(None, mv.from, "R-JKF-003: a drop states no origin");
    assert_eq!(
        Some(Kind::HI),
        mv.capture,
        "4五 is empty, but there is no origin to work that out from"
    );
    assert_eq!(Kind::KA, mv.piece, "the piece dropped, from the record");
}

// A promotion with nowhere to go is the one the record cannot have right: a
// gold has no promoted form, so there is no move to keep. That has to be an
// error naming the ply — the reader silently unpromoting it would leave a
// record that disagrees with the file for the rest of the game.
#[test]
fn a_promotion_the_piece_has_no_form_for_is_an_error() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ６八金成(69)
";
    let err = crate::parser::parse_kif_str(kif).expect_err("refuses");
    let text = err.to_string();
    assert!(text.contains("６八"), "the error names the move: {text}");
    assert!(text.contains("at ply 1"), "and which ply it is: {text}");
}

// R-NOT-005: `false` is worth recording only where promoting was on the
// table — a promotable piece with the enemy camp at one end of the move.
// Everywhere else the field stays empty, which is what the writers read to
// decide whether KI2 spells 不成.
#[test]
fn a_move_that_did_not_promote_records_false_only_in_the_enemy_camp() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２二角(88)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    assert_eq!(
        None,
        jkf.moves[1].move_.expect("a move").promote,
        "7七から7六は敵陣に無関係"
    );
    assert_eq!(
        Some(false),
        jkf.moves[3].move_.expect("a move").promote,
        "2二は敵陣なので不成であることに意味がある"
    );
}

// `同` carries no destination of its own — it means the square the previous
// move went to (R-NOT-002) — and only normalization can fill it in. Stopping
// at the special move node leaves `to` at (0,0), which no writer can spell, so
// saving a game that was interrupted and resumed fails outright.
#[test]
fn a_move_after_an_interruption_is_still_normalized() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２二角成(88)
   4 中断
   5 同　銀(31)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let mv = jkf.moves[5].move_.expect("a move");
    assert_eq!(PlaceFormat { x: 2, y: 2 }, mv.to, "`同` resolves to 2二");
    assert_eq!(Color::White, mv.color);
    assert_eq!(Some(Kind::UM), mv.capture, "it takes the horse");
    // And the record can be written back out.
    use crate::converter::{ToKi2, ToKif};
    assert!(jkf.try_to_kif_owned().is_ok());
    assert!(jkf.try_to_ki2_owned().is_ok());
}

// `直` is the only way to read `▲５八金直` when more than one gold reaches
// the square, and it is the one arm of `resolve_from`'s match that the
// 左/右 rewrite did not touch. Left unfiltered, the move comes back ambiguous
// and the fork holding it is dropped.
#[test]
fn a_gold_moving_straight_up_is_read_from_直() {
    let kif = "手合割：その他
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
| 玉 ・ ・ 金 金 金 ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 ５八金(59)
";
    use crate::converter::ToKi2;
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(ki2.contains("▲５八金直"), "{ki2:?}");
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(
        Some(PlaceFormat { x: 5, y: 9 }),
        back.moves[1].move_.expect("a move").from,
    );
}

/// Builds a board holding `pieces`, plus a king for each side, and reads
/// the KIF for `mv` played from it.
fn from_board(pieces: &[(usize, usize, Color, Kind)], mv: &str) -> JsonKifuFormat {
    let mut board = String::new();
    let mut cells = [[None; 9]; 9];
    // Kings in the corners, out of the way of the squares these tests use.
    cells[0][8] = Some((Color::Black, Kind::OU));
    cells[0][0] = Some((Color::White, Kind::OU));
    for &(x, y, color, kind) in pieces {
        cells[x - 1][y - 1] = Some((color, kind));
    }
    for rank in 1..=9usize {
        board.push('|');
        for file in (1..=9usize).rev() {
            match cells[file - 1][rank - 1] {
                None => board.push_str(" ・"),
                Some((color, kind)) => {
                    board.push(if color == Color::Black { ' ' } else { 'v' });
                    board.push_str(match kind {
                        Kind::KI => "金",
                        Kind::GI => "銀",
                        Kind::TO => "と",
                        Kind::KA => "角",
                        Kind::UM => "馬",
                        Kind::HI => "飛",
                        Kind::RY => "龍",
                        Kind::OU => "玉",
                        _ => unreachable!("only the kinds these tests place"),
                    });
                }
            }
        }
        board.push('|');
        board.push_str(["一", "二", "三", "四", "五", "六", "七", "八", "九"][rank - 1]);
        board.push('\n');
    }
    let kif = format!(
        "手合割：その他\n後手の持駒：なし\n  ９ ８ ７ ６ ５ ４ ３ ２ １\n\
+---------------------------+\n{board}+---------------------------+\n\
先手の持駒：なし\n先手番\n手数----指手---------消費時間--\n   1 {mv}\n"
    );
    crate::parser::parse_kif_str(&kif).unwrap_or_else(|e| panic!("{kif}\n{e}"))
}

// R-NOT-004 stage 2: 直 is for a gold-like piece going straight *up*. A gold
// directly behind the destination is not 直 — it would be 引 — so a reader
// that only checks the file keeps it as a candidate and the move comes back
// ambiguous. The writer already knew; only the reader did not, which is what
// a second copy of the rule buys.
//
// 5七 / 5九 / 4九 all reach 5八, and the move is 5九→5八.
#[test]
fn a_gold_behind_the_destination_is_not_直() {
    use crate::converter::ToKi2;
    let jkf = from_board(
        &[
            (5, 7, Color::Black, Kind::KI),
            (5, 9, Color::Black, Kind::KI),
            (4, 9, Color::Black, Kind::KI),
        ],
        "５八金(59)",
    );
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(ki2.contains("▲５八金直"), "{ki2:?}");
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(
        Some(PlaceFormat { x: 5, y: 9 }),
        back.moves[1].move_.expect("a move").from,
    );
}

// Every arrangement of gold-like pieces around one square, written and read
// back. Enumeration is what found the 直 case above: it is not a shape
// anyone thinks to write by hand, and the corpus has none of it.
//
// Either the notation can name the mover — and then reading has to land on
// the same square — or it cannot, and then the write has to fail rather than
// produce a record that will not open (R-NOT-004 stage 3, R-KI2-003).
#[test]
fn every_arrangement_of_golds_round_trips_or_refuses() {
    use crate::converter::ToKi2;
    // The squares a Black gold reaches 5八 from.
    const AROUND: [(usize, usize); 6] = [(5, 7), (4, 8), (6, 8), (4, 9), (6, 9), (5, 9)];
    let mut spelled = 0;
    let mut refused = 0;
    for kind in [Kind::KI, Kind::TO] {
        for mask in 0u32..(1 << AROUND.len()) {
            let placed: Vec<_> = AROUND
                .iter()
                .enumerate()
                .filter(|(i, _)| mask & (1 << i) != 0)
                .map(|(_, &(x, y))| (x, y, Color::Black, kind))
                .collect();
            if placed.len() < 2 {
                continue;
            }
            for &(x, y, ..) in &placed {
                let jkf = from_board(&placed, &format!("５八{}({x}{y})", kif_word(kind)));
                let Ok(ki2) = jkf.try_to_ki2_owned() else {
                    refused += 1;
                    continue;
                };
                spelled += 1;
                let back = crate::parser::parse_ki2_str(&ki2).unwrap_or_else(|e| {
                    panic!("wrote {ki2:?} from {placed:?} and could not read it: {e}")
                });
                assert_eq!(
                    Some(PlaceFormat {
                        x: x as u8,
                        y: y as u8
                    }),
                    back.moves[1].move_.expect("a move").from,
                    "{ki2:?} from {placed:?}"
                );
            }
        }
    }
    // Gold-like pieces reach a square from at most six others, and 左/直/右
    // crossed with 上/寄/引 names nine — so every arrangement of them is
    // spellable. The unspellable case needs a piece that comes from further
    // away; `a_move_the_notation_cannot_spell_is_not_written` has it.
    assert_eq!(0, refused, "a gold arrangement could not be spelled");
    assert!(spelled > 100, "only {spelled} arrangements were exercised");
}

fn kif_word(kind: Kind) -> &'static str {
    match kind {
        Kind::KI => "金",
        Kind::TO => "と",
        _ => unreachable!("only the kinds this test places"),
    }
}

// R-NOT-004 stage 3: three bishops reaching one square cannot be told apart
// — the notation has 左 and 右 and nothing else for them. Writing the move
// bare produces KI2 nothing can read (R-KI2-003), and the record being saved
// is the only copy, so the write has to fail instead.
#[test]
fn a_move_the_notation_cannot_spell_is_not_written() {
    use crate::converter::ToKi2;
    let jkf = from_board(
        &[
            (1, 7, Color::Black, Kind::KA),
            (3, 1, Color::Black, Kind::KA),
            (6, 2, Color::Black, Kind::KA),
        ],
        "５三角(31)",
    );
    // Naming the move is the point: the consumer has to be able to tell
    // the user which move it could not save.
    assert_eq!(
        "the notation cannot tell this move apart: ３一→５三",
        jkf.try_to_ki2_owned()
            .expect_err("three bishops reach 5三")
            .to_string()
    );
}

// A fork whose moves cannot be replayed is still a fork. Dropping it
// returns `Ok` with the record one variation shorter, and the next save
// writes the shortened record over the original — the loss is permanent and
// nothing ever said so. Kifu holding illegal moves are valid input
// (R-RULE-002).
//
// The moves after `中断` here do not continue the position before it, which
// is what a resumed game looks like.
#[test]
fn a_fork_that_cannot_be_replayed_is_kept() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 ３四歩(33)
   3 ２六歩(27)
   4 ８四歩(83)

変化：2手
   2 ８四歩(83)
   3 中断
   4 ９九角(88)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let forks = jkf.moves[2].forks.as_ref().expect("a fork at ply 2");
    assert_eq!(1, forks.len(), "the fork survives normalization");
    assert_eq!(3, forks[0].len(), "and keeps all three of its nodes");
}

// JKF says a drop by leaving `from` out (R-JKF-003), so an origin that is
// not a square on the board cannot be quietly turned into one — the square
// is lost and the record comes back saying the piece came from the hand.
// `(00)` is CSA's spelling for a drop, so it is a shape a converter written
// by someone else really does produce.
#[test]
fn an_origin_off_the_board_is_an_error_not_a_drop() {
    for origin in ["(00)", "(10)", "(09)"] {
        let kif = format!("手合割：平手\n手数----指手---------消費時間--\n   1 ７六歩{origin}\n");
        assert!(
            crate::parser::parse_kif_str(&kif).is_err(),
            "{origin} was accepted"
        );
    }
}

// The KIF reader rejects an off-board origin itself, so the normalizer's
// own refusal is only reachable from a source that states `from` as data —
// JKF, which the consumer's save path feeds (R-REQ-002).
//
// The error has to name the square that could not be read. Telling "this
// record holds one unreadable coordinate" from "this is not a kifu at all"
// is a decision the caller makes on the variant, which is why the nesting
// is kept as errors instead of flattened to a message.
#[test]
fn an_origin_off_the_board_names_the_square_it_could_not_read() {
    for (x, y) in [(10, 1), (1, 10), (0, 9)] {
        let json = format!(
            r#"{{"header":{{}},"initial":{{"preset":"HIRATE"}},"moves":[{{}},
                   {{"move":{{"color":0,"from":{{"x":{x},"y":{y}}},"to":{{"x":7,"y":6}},"piece":"FU"}}}}]}}"#
        );
        let err = crate::parser::parse_jkf_str(&json).expect_err("({x},{y}) was accepted");
        let crate::error::ParseError::Normalize(NormalizeError {
            ply: 1,
            kind: NormalizeErrorKind::Convert(inner),
        }) = err
        else {
            panic!("({x},{y}) gave {err:?}, not a conversion error");
        };
        assert_eq!(ConvertError::InvalidSquare((x, y)), *inner, "({x},{y})");
    }
}

// R-KIF-008: the move's own time is 分:秒, and the minutes have no stated
// limit. An hour has to fold into them — writing only `time.now.m` drops it
// and leaves the file disagreeing with its own running total.
#[test]
fn an_hour_of_thinking_survives_a_kif_round_trip() {
    use crate::converter::ToKif;
    let csa = "V2.2\nPI\n+\n+7776FU\nT3723\n";
    let jkf = crate::parser::parse_csa_str(csa).expect("parses CSA");
    assert_eq!(
        Some(1),
        jkf.moves[1].time.expect("a time").now.h,
        "3723 seconds is an hour and change"
    );
    let kif = jkf.try_to_kif_owned().expect("writes KIF");
    assert!(kif.contains("(62:03/01:02:03)"), "{kif:?}");
    let back = crate::parser::parse_kif_str(&kif).expect("reads back");
    let now = back.moves[1].time.expect("a time").now;
    assert_eq!(
        3723,
        u32::from(now.h.unwrap_or_default()) * 3600 + u32::from(now.m) * 60 + u32::from(now.s),
        "the seconds spent on the move"
    );
}

// R-KIF-008 puts no limit on the minutes a move took, and a title game with
// nine hours on the clock produces `256:00` and more. A field too narrow to
// hold it makes the time column unparseable, and `opt` then reports the move
// as having no time at all — so saving it back drops the column.
#[test]
fn a_move_longer_than_a_byte_of_minutes_keeps_its_time() {
    use crate::converter::ToKif;
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77) (300:00/05:00:00)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let now = jkf.moves[1].time.expect("a time").now;
    assert_eq!((300, 0), (now.m, now.s), "300 minutes");
    assert!(
        jkf.try_to_kif_owned()
            .expect("writes KIF")
            .contains("(300:00/05:00:00)"),
        "the column survives a round trip"
    );
}

// A move the board turns out not to explain keeps whatever it said. The
// normalization runs before anyone knows the board holds the move, so it has
// to run on a copy: otherwise a 銀 comes back as the 金 that stood on that
// square in the position *before* the interruption, and the running clock
// keeps adding to one side.
#[test]
fn a_move_the_board_cannot_explain_is_left_alone() {
    // 6九 holds a gold at the start, so normalizing 6八銀(69) against the
    // pre-中断 board would rewrite the piece.
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77) ( 0:10/00:00:10)
   2 中断
   3 ６八銀(69) ( 0:20/00:00:20)
   4 ３四歩(33) ( 0:30/00:00:30)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    assert_eq!(
        Kind::GI,
        jkf.moves[3].move_.expect("a move").piece,
        "the piece the file named"
    );
    for (i, want) in [(3, 20), (4, 30)] {
        assert_eq!(
            TimeFormat {
                h: Some(0),
                m: 0,
                s: want
            },
            jkf.moves[i].time.expect("a time").total,
            "the total the file stated at ply {i}"
        );
    }
}

// The board is dropped at the *first* move a special move's position cannot
// explain, and stays dropped. Without that, every later move is normalized
// against a board the game left behind.
#[test]
fn the_board_stays_dropped_after_a_special() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
   4 ６八銀(69)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    assert_eq!(
        Kind::GI,
        jkf.moves[4].move_.expect("a move").piece,
        "the move after the one that lost the board is untouched too"
    );
}

// A fork hanging off a node past the point the board was lost has no
// position to be normalized against either. Normalizing it anyway rewrites
// its moves — `correct_color` alone flips the side — against a board they
// never came from.
//
// The fork has to leave an *odd* ply. The board was lost with White to
// move, so at an even ply the stale board and the ply number name the same
// side and normalizing the fork anyway makes no visible difference.
#[test]
fn a_fork_past_the_lost_board_is_left_alone() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
   4 ８四歩(83)
   5 ２六歩(27)

変化：5手
   5 ４四歩(43)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let fork = &jkf.moves[5].forks.as_ref().expect("a fork")[0];
    let mv = fork[0].move_.expect("a move");
    assert_eq!(Color::Black, mv.color, "the side the ply number gave it");
    assert_eq!(Some(PlaceFormat { x: 4, y: 3 }), mv.from);
}

// R-RULE-002: a kifu recording an illegal move is valid input, and
// `is_valid` is exactly what leaves such a move out of the candidate scan.
// Left out, the suffix describes one of the *other* pieces, so the record
// comes back saying a piece moved that never did.
//
// KI2 carries no origin (R-KI2-003), so reading an illegal move back to its
// square is not possible either way — what the suffix must not do is name
// the wrong piece.
#[test]
fn an_illegal_move_is_spelled_for_the_piece_that_made_it() {
    use crate::converter::ToKi2;
    // 4九 cannot reach 5五; the record says it did. 5六 can, and would be
    // spelled with no suffix at all if it were the only candidate.
    let jkf = from_board(
        &[
            (4, 9, Color::Black, Kind::KI),
            (5, 6, Color::Black, Kind::KI),
        ],
        "５五金(49)",
    );
    let ki2 = jkf
        .try_to_ki2_owned()
        .expect("an illegal move is still writable");
    assert!(ki2.contains("▲５五金右"), "{ki2:?}");
}

// The other half: when no legal candidate shares the illegal move's motion,
// leaving it out of the set makes the notation unable to name anything — and
// since that is now an error, one illegal move would stop the whole record
// from being saved.
#[test]
fn an_illegal_move_does_not_stop_the_record_from_being_written() {
    use crate::converter::ToKi2;
    let jkf = from_board(
        &[
            (5, 9, Color::Black, Kind::KI),
            (4, 5, Color::Black, Kind::KI),
            (6, 5, Color::Black, Kind::KI),
        ],
        "５五金(59)",
    );
    assert!(
        jkf.try_to_ki2_owned().is_ok(),
        "one illegal move must not cost the whole record"
    );
}

// Same shape on the main line: a game that resumed after `中断` and went on
// from somewhere else. Refusing the whole record would take a readable kifu
// out of the consumer's index entirely.
#[test]
fn a_resumed_game_that_diverges_is_still_readable() {
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    assert_eq!(4, jkf.moves.len(), "every node is kept: {:?}", jkf.moves);
}

// JKF says a move is a drop by leaving `from` out (R-JKF-003). Reading that
// as "the origin is missing, look it up" turns the drop into whichever board
// piece could have reached the square: that piece comes off the board, the
// hand is never spent, and a few moves later the square it left is empty.
//
// Only KI2 has moves whose origin the notation does not carry, and its
// reader says so with `ORIGIN_UNSTATED`.
#[test]
fn a_drop_stays_a_drop_through_normalize() {
    // Black has a gold on 4九 that reaches 3八, and a gold in hand. The
    // record drops the one in hand; the one on the board must not move.
    let mut state = StateFormat {
        color: Color::Black,
        board: [[Piece {
            color: None,
            kind: None,
        }; 9]; 9],
        hands: [Hand::default(); 2],
    };
    let put = |board: &mut [[Piece; 9]; 9], x: usize, y: usize, color, kind| {
        board[x - 1][y - 1] = Piece {
            color: Some(color),
            kind: Some(kind),
        };
    };
    put(&mut state.board, 5, 9, Color::Black, Kind::OU);
    put(&mut state.board, 4, 9, Color::Black, Kind::KI);
    put(&mut state.board, 5, 1, Color::White, Kind::OU);
    state.hands[0].KI = 1;

    let mut jkf = JsonKifuFormat {
        initial: Some(Initial {
            preset: Preset::PresetOther,
            data: Some(state),
        }),
        moves: vec![
            MoveFormat::default(),
            MoveFormat {
                move_: Some(MoveMoveFormat {
                    color: Color::Black,
                    from: None,
                    to: PlaceFormat { x: 3, y: 8 },
                    piece: Kind::KI,
                    same: None,
                    promote: None,
                    capture: None,
                    relative: None,
                }),
                ..Default::default()
            },
        ],
        ..Default::default()
    };
    jkf.normalize().expect("normalizes");
    assert_eq!(
        None,
        jkf.moves[1].move_.expect("a move").from,
        "the drop was rewritten as a board move"
    );
    // And the board gold is still on 4九, not on 3八.
    let pos = jkf.initial_position().expect("a position");
    let mut pos = pos;
    pos.make_move(shogi_core::Move::try_from(&jkf.moves[1].move_.unwrap()).expect("a move"))
        .expect("the drop applies");
    assert!(
        pos.piece_at(shogi_core::Square::new(4, 9).expect("a square"))
            .is_some(),
        "the gold on 4九 must still be there"
    );
}

// Cumulative times are summed from values the file states, so they outgrow
// a byte. Wrapping would put a small, ordinary-looking number where a broken
// one belongs, and nothing downstream could tell.
#[test]
fn a_cumulative_time_past_the_hour_limit_does_not_wrap() {
    // Black plays twice at 255 hours each: 510, which is 254 in a `u8`.
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77) (255:00:00/255:00:00)
   2 ３四歩(33) ( 0:00/00:00:00)
   3 ２六歩(27) (255:00:00/255:00:00)
";
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let total = jkf.moves[3].time.expect("a time").total;
    assert_eq!(Some(510), total.h, "hours add up rather than wrap");
}

// 馬 reaches a square from that square's own file, so one of two candidates
// can be named 右 while standing on the destination's file. Comparing the
// origin with the destination instead of the candidates with each other
// leaves no candidate and the move reads back as ambiguous — which is how a
// `.ki2` written by any of the tools in use became unreadable here.
#[test]
fn a_horse_on_the_destination_file_still_takes_a_left_or_right() {
    let kif = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ 馬 馬 ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 ４四馬(43)
";
    use crate::converter::ToKi2;
    let jkf = crate::parser::parse_kif_str(kif).expect("parses");
    let ki2 = jkf.try_to_ki2_owned().expect("writes KI2");
    assert!(ki2.contains("▲４四馬右"), "wrote {ki2:?}");
    let back = crate::parser::parse_ki2_str(&ki2).expect("reads back");
    assert_eq!(
        Some(PlaceFormat { x: 4, y: 3 }),
        back.moves[1].move_.expect("a move").from,
    );
}

/// Two black bishops on 7a and 3a, both able to reach 5c, so every move to
/// 5c needs a 左/右 disambiguator (R-NOT-004). `{mv}` is the move line.
fn ambiguous_bishop_kif(mv: &str) -> String {
    format!(
        "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ 角 ・v玉 ・ 角 ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・ ・ ・ ・ ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ ・ ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 {mv}   ( 0:00/00:00:00)
"
    )
}

// R-NOT-004 / R-NOT-005: the promotion suffix must not hide the
// disambiguator. `不成` is covered too — stripping only `成` leaves it
// broken, and a KI2 written without the disambiguator cannot be read back.
#[test]
fn relative_survives_promotion_suffix() {
    for (mv, want) in [
        ("５三角成(71)", Relative::L),
        ("５三角成(31)", Relative::R),
        ("５三角(71)", Relative::L),
        ("５三角(31)", Relative::R),
    ] {
        let src = ambiguous_bishop_kif(mv);
        let mut jkf = crate::parser::parse_kif_str(&src)
            .unwrap_or_else(|e| panic!("failed to parse {mv}: {e}"));
        jkf.populate_relative()
            .unwrap_or_else(|e| panic!("failed to populate {mv}: {e}"));
        assert_eq!(
            Some(want),
            jkf.moves[1].move_.expect("a move").relative,
            "relative for {mv}"
        );
    }
}

// The error a caller shows a user has to point back into the file. An
// internal square index does not: `Square(61)` is neither the file nor the
// rank, so nobody can find the move it is talking about.
#[test]
fn a_move_that_will_not_apply_is_named_in_shogi_coordinates() {
    let record = |mv: &str| {
        format!(r#"{{"header":{{}},"initial":{{"preset":"HIRATE"}},"moves":[{{}},{mv}]}}"#)
    };
    for (mv, want) in [
        (
            // 5五 is empty at the start, so nothing there can have moved.
            r#"{"move":{"color":0,"from":{"x":5,"y":5},"to":{"x":5,"y":4},"piece":"FU"}}"#,
            "failed to normalize: no pieces at ５五 at ply 1",
        ),
        (
            // A drop with an empty hand: JKF says a drop by leaving `from`
            // out (R-JKF-003).
            r#"{"move":{"color":0,"to":{"x":5,"y":5},"piece":"FU"}}"#,
            "failed to normalize: invalid move: ５五歩打 at ply 1",
        ),
    ] {
        let err = crate::parser::parse_jkf_str(&record(mv)).expect_err("{mv} was accepted");
        assert_eq!(want, err.to_string());
    }
}

// GAP-016: `normalize_moves` treats a move that will not apply after an
// special move as the end of what it knows about the board (R-RULE-002).
// `populate_relative_moves` has to follow the same rule, or a record the
// parser accepted and `to_ki2` can write is an `Err` from the one call in
// between. The rule belongs to the walk, not to one of the two functions
// that do it, or the next walk added grows the same hole.
#[test]
fn filling_in_relative_accepts_what_normalizing_accepted() {
    use crate::converter::ToKi2;
    // The moves after `中断` do not continue the position before it, which
    // is what a game that was interrupted and resumed looks like.
    let kif = "手合割：平手
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
   4 ８四歩(83)
";
    let mut jkf = crate::parser::parse_kif_str(kif).expect("parses");
    jkf.populate_relative()
        .expect("what the parser accepted, this has to accept");
    assert!(
        jkf.try_to_ki2_owned().is_ok(),
        "and the writer still writes it"
    );
}

// Past the point where the board was lost, nothing is filled in. A suffix
// says which of several pieces made the move (R-NOT-004), so one read off a
// position the move never came from is a wrong answer written into the
// record — and `relative` is a field the writers and other tools believe.
//
// The board here stops at ply 1. Both White golds reach 5三, so a walk that
// kept going gives ply 5 a 右 that names a piece nobody moved.
#[test]
fn no_suffix_is_invented_past_the_lost_board() {
    let kif = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・v金 ・v金 ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ 歩 ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ９九角(88)
   4 ８八角(99)
   5 ５三金(63)
";
    let mut jkf = crate::parser::parse_kif_str(kif).expect("parses");
    jkf.populate_relative().expect("fills in what it can");
    assert_eq!(
        None,
        jkf.moves[5].move_.expect("a move").relative,
        "{:?}",
        jkf.moves
    );
}

// The move that loses the board gets no suffix either, which is why the
// suffix is read before the move is played but written only after.
//
// A drop is where the two orders differ. `打` is written when a piece on
// the board could have reached the square as well (R-NOT-003), so it is
// read off the position — while the drop itself fails, because the hand it
// names is empty.
#[test]
fn no_suffix_is_invented_for_the_move_that_loses_the_board() {
    let kif = "手合割：その他
後手の持駒：なし
  ９ ８ ７ ６ ５ ４ ３ ２ １
+---------------------------+
| ・ ・ ・ ・v玉 ・ ・ ・ ・|一
| ・ ・ ・ ・ ・ ・ ・ ・ ・|二
| ・ ・ ・v金 ・ ・ ・ ・ ・|三
| ・ ・ ・ ・ ・ ・ ・ ・ ・|四
| ・ ・ ・ ・ ・ ・ ・ ・ ・|五
| ・ ・ ・ ・ ・ ・ ・ ・ ・|六
| ・ ・ 歩 ・ ・ ・ ・ ・ ・|七
| ・ ・ ・ ・ ・ ・ ・ ・ ・|八
| ・ ・ ・ ・ 玉 ・ ・ ・ ・|九
+---------------------------+
先手の持駒：なし
先手番
手数----指手---------消費時間--
   1 ７六歩(77)
   2 中断
   3 ５三金打
";
    let mut jkf = crate::parser::parse_kif_str(kif).expect("parses");
    jkf.populate_relative().expect("fills in what it can");
    let mv = jkf.moves[3].move_.expect("a move");
    assert_eq!(None, mv.from, "it is a drop (R-JKF-003)");
    assert_eq!(None, mv.relative, "and no hand held the piece it drops");
}

// Which move failed is the other half. A caller handed a directory of kifu
// and told "invalid move" has nothing to show; told the ply, it has a line.
// The ply counts the way JKF indexes `moves`, where index 0 is the initial
// position's slot and the first move is 1 (R-JKF-001).
#[test]
fn a_normalization_error_says_which_move_it_is_about() {
    let opening = [(7, 7, 7, 6), (3, 3, 3, 4), (2, 7, 2, 6), (8, 3, 8, 4)]
            .iter()
            .enumerate()
            .map(|(i, (fx, fy, tx, ty))| {
                format!(
                    r#",{{"move":{{"color":{},"from":{{"x":{fx},"y":{fy}}},"to":{{"x":{tx},"y":{ty}}},"piece":"FU"}}}}"#,
                    i % 2
                )
            })
            .collect::<String>();
    // A fifth move from a square that has been empty all along.
    let broken = r#",{"move":{"color":0,"from":{"x":5,"y":5},"to":{"x":5,"y":4},"piece":"FU"}}"#;
    let json = format!(
        r#"{{"header":{{}},"initial":{{"preset":"HIRATE"}},"moves":[{{}}{opening}{broken}]}}"#
    );
    let err = crate::parser::parse_jkf_str(&json).expect_err("5五 is empty");
    assert_eq!(
        "failed to normalize: no pieces at ５五 at ply 5",
        err.to_string()
    );
}

#[test]
fn normalize_moves_empty() {
    let pos = PartialPosition::startpos();
    assert!(normalize_moves(
        &mut [],
        1,
        pos,
        [TimeFormat::default(); 2],
        Normalization::RECORDED
    )
    .is_ok());
}

#[test]
fn normalize_moves_invalid_color() {
    let mmf = MoveMoveFormat {
        color: Color::Black,
        piece: Kind::FU,
        from: Some(PlaceFormat { x: 7, y: 7 }),
        to: PlaceFormat { x: 7, y: 6 },
        promote: None,
        capture: None,
        relative: None,
        same: None,
    };
    // Correct color should succeed
    {
        let pos = PartialPosition::startpos();
        assert!(
            normalize_moves(
                &mut [MoveFormat {
                    move_: Some(mmf),
                    ..Default::default()
                }],
                1,
                pos,
                [TimeFormat::default(); 2],
                Normalization::RECORDED,
            )
            .is_ok(),
            "normalize should succeed"
        );
    }
    // A recorded color that disagrees with the position is an error
    {
        let pos = PartialPosition::startpos();
        assert!(
            normalize_moves(
                &mut [MoveFormat {
                    move_: Some(MoveMoveFormat {
                        color: Color::White,
                        ..mmf
                    }),
                    ..Default::default()
                }],
                1,
                pos,
                [TimeFormat::default(); 2],
                Normalization::RECORDED,
            )
            .is_err(),
            "normalize should fail with InvalidColor"
        );
    }
    // A color guessed from the ply is overwritten with the side to move
    {
        let pos = PartialPosition::startpos();
        let mut moves = [MoveFormat {
            move_: Some(MoveMoveFormat {
                color: Color::White,
                ..mmf
            }),
            ..Default::default()
        }];
        assert!(
            normalize_moves(
                &mut moves,
                1,
                pos,
                [TimeFormat::default(); 2],
                Normalization::KI2
            )
            .is_ok(),
            "normalize should succeed (color auto-corrected)"
        );
        assert_eq!(
            moves[0].move_.as_ref().unwrap().color,
            Color::Black,
            "color should be corrected to Black"
        );
    }
}
