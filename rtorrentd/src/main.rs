use std::{env, fs};

fn main() {
    let args: Vec<String> = env::args().collect();

    let contents = fs::read("./test.torrent").expect("Should have been able to read the file");

    let s = librtorrent::bencode::decode_to_vec(&contents).unwrap();

    println!("{s:?}");
}
