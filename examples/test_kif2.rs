use shogi_kifu_converter::parser::{parse_kif_file, parse_kif_str};
fn main() {
    // Test parse_kif_str (direct string)
    let content = std::fs::read_to_string("test/initial_position_2.kif").unwrap();
    match std::panic::catch_unwind(|| parse_kif_str(&content)) {
        Ok(Ok(jkf)) => println!("parse_kif_str OK: {} moves", jkf.moves.len()),
        Ok(Err(e)) => println!("parse_kif_str Error: {e:?}"),
        Err(panic) => println!("parse_kif_str PANIC: {panic:?}"),
    }
    // Test parse_kif_file (with UTF-8 fallback)
    match parse_kif_file("test/initial_position_2.kif") {
        Ok(jkf) => println!("parse_kif_file OK: {} moves", jkf.moves.len()),
        Err(e) => println!("parse_kif_file Error: {e:?}"),
    }
}
