use shogi_kifu_converter::converter::ToKif;
use shogi_kifu_converter::error::ParseError;
use shogi_kifu_converter::parser::parse_csa_file;
use std::env;

fn main() -> Result<(), ParseError> {
    let argv = env::args().collect::<Vec<_>>();
    if argv.len() != 2 {
        eprintln!("Usage: {} <CSA file>", argv[0]);
        std::process::exit(1);
    }
    let jkf = parse_csa_file(&argv[1])?;
    // Not `to_kif_owned`: that one hands back whatever was written before
    // the failure, which looks like a complete record and is not one.
    match jkf.try_to_kif_owned() {
        Ok(text) => print!("{text}"),
        Err(_) => {
            eprintln!("{}: the record cannot be spelled in KIF", argv[1]);
            std::process::exit(1);
        }
    }
    Ok(())
}
