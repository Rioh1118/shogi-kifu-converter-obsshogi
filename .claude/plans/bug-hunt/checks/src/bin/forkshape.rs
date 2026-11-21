use shogi_kifu_converter::{jkf::*, parser::{parse_file, parse_str}, Format};
fn shape(ms: &[MoveFormat], ply: usize, depth: usize, out: &mut Vec<String>) {
    for (i, m) in ms.iter().enumerate() {
        if let Some(f) = &m.forks {
            for (k, fk) in f.iter().enumerate() {
                let first = fk.first().and_then(|x| x.move_).map(|x| format!("{}{}{:?}", x.to.x, x.to.y, x.piece)).unwrap_or_default();
                out.push(format!("{}ply{} fork{} starts {}", "  ".repeat(depth), ply + i, k, first));
                shape(fk, ply + i, depth + 1, out);
            }
        }
    }
}
fn main() {
    let p = std::env::args().nth(1).unwrap();
    let j = parse_file(&p).unwrap();
    let mut a = vec![]; shape(&j.moves, 0, 0, &mut a);
    let b2 = parse_str(Format::Kif, &j.try_to_string(Format::Kif).unwrap()).unwrap();
    let mut b = vec![]; shape(&b2.moves, 0, 0, &mut b);
    println!("--- original"); for x in &a { println!("{x}"); }
    println!("--- after KIF"); for x in &b { println!("{x}"); }
}
