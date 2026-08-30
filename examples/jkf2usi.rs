use shogi_kifu_converter::error::ParseError;
use shogi_kifu_converter::parser::parse_jkf_file;
use std::env;

fn main() -> Result<(), ParseError> {
    let argv = env::args().collect::<Vec<_>>();
    if argv.len() != 2 {
        eprintln!("Usage: {} <JKF file>", argv[0]);
        std::process::exit(1);
    }
    let jkf = parse_jkf_file(&argv[1])?;
    // Not `ToUsi::to_usi_owned`: a record holding an illegal move is valid input
    // (R-RULE-002) and cannot be replayed, and that default method turns the
    // failure into a panic or an empty string.
    match jkf.try_to_usi_owned() {
        Ok(usi) => println!("{usi}"),
        Err(_) => {
            eprintln!("{}: the record cannot be replayed into a position", argv[1]);
            std::process::exit(1);
        }
    }
    Ok(())
}
