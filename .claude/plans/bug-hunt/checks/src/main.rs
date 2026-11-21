//! OSS のテスト用棋譜を全て読み、読めないもの・panic するものを列挙する。
use shogi_kifu_converter::converter::{ToCsa, ToKi2, ToKif};
use shogi_kifu_converter::parser::parse_file;
fn main() {
    for path in std::env::args().skip(1) {
        let r = std::panic::catch_unwind(|| parse_file(&path));
        let moves = |j: &shogi_kifu_converter::JKF| j.moves.iter().filter(|m| m.move_.is_some()).count();
        match r {
            Err(_) => println!("PANIC\t{path}"),
            Ok(Err(e)) => println!("ERR\t{path}\t{}", e.to_string().lines().next().unwrap_or("").chars().take(110).collect::<String>()),
            Ok(Ok(j)) => {
                let mut w = vec![];
                for (f, t) in [("kif", j.try_to_kif_owned()), ("ki2", j.try_to_ki2_owned()), ("csa", j.try_to_csa_owned())] {
                    if let Err(e) = t { w.push(format!("{f}:{e}")); }
                }
                println!("OK\t{path}\tmoves={}\t{}", moves(&j), w.join(" "));
            }
        }
    }
}
