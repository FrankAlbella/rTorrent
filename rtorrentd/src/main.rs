use std::{fs, path::PathBuf};

use librtorrent::{bencode::BencodeType, meta_info::FromBencodemap, meta_info::MetaInfo};

#[tokio::main]
async fn main() {
    // let args: Vec<String> = env::args().collect();
    //let file_path = &args[1];

    let file_path = PathBuf::from("test/torrent_files/debian-13.1.0-amd64-netinst.iso.torrent");
    let contents = fs::read(file_path).expect("Should have been able to read the file");

    let bencode_vec = librtorrent::bencode::decode_to_vec(&contents).unwrap();

    let mut iter = bencode_vec.iter();

    match iter.next() {
        Some(x) => match x {
            BencodeType::Dictionary(x) => {
                let data = MetaInfo::from_bencodemap(x).unwrap();

                //dbg!(data.clone());

                dbg!(librtorrent::tracker::send_get_request(&data).await.unwrap());
            }
            _ => println!(),
        },
        _ => println!("Invalid torrent file!"),
    }
}
