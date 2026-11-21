//! 引数の棋譜を読み、JKF の JSON を out/<名前>.json に書く。読めなければ書かない。
fn main() {
    let out = std::env::args().nth(1).unwrap();
    for path in std::env::args().skip(2) {
        if let Ok(Ok(j)) = std::panic::catch_unwind(|| shogi_kifu_converter::parser::parse_file(&path)) {
            let name = std::path::Path::new(&path).file_name().unwrap().to_string_lossy().to_string();
            std::fs::write(format!("{out}/{name}.json"), serde_json::to_string(&j).unwrap()).unwrap();
        }
    }
}
