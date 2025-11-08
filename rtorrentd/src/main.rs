use std::env;

fn main() {
    let args: Vec<String> = env::args().collect();

    let s = librtorrent::bencode::decode_to_vec(&args[1]);

    println!("{args:?}");
    dbg!(s.unwrap());
}
