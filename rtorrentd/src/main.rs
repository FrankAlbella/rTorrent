use std::{env, fs, path::PathBuf};

use librtorrent::{bencode::BencodeType, meta_info::FromBencodemap, meta_info::MetaInfo};

fn main() {
    // let args: Vec<String> = env::args().collect();
    //let file_path = &args[1];

    let file_path = PathBuf::from("./archlinux-2025.11.01-x86_64.iso.torrent");
    let contents = fs::read(file_path).expect("Should have been able to read the file");

    let bencode_vec = librtorrent::bencode::decode_to_vec(&contents).unwrap();

    let mut iter = bencode_vec.iter();

    match iter.next() {
        Some(x) => match x {
            BencodeType::Dictionary(x) => {
                let data = MetaInfo::from_bencodemap(x);

                dbg!(data.unwrap());
            }
            _ => println!(),
        },
        _ => println!("Invalid torrent file!"),
    }
}
