use std::{fs, path::PathBuf, sync::Arc};

use librtorrent::{
    bencode::BencodeType,
    handshake::Handshake,
    meta_info::{FromBencodemap, MetaInfo},
};

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
                let data = Arc::new(MetaInfo::from_bencodemap(x).unwrap());
                let response = librtorrent::tracker::send_get_request(&data).await;
                let mut handles = Vec::new();

                if let Ok(res) = response {
                    let mut peers = res.peers.iter();

                    if let Some(peers_vec) = peers.next() {
                        let mut peers_iter = peers_vec.iter();
                        while let Some(peer) = peers_iter.next() {
                            let hash_clone = data.hash.clone();
                            let peer_clone = peer.clone();
                            handles.push(tokio::spawn(async move {
                                peer_clone
                                    .connect(&Handshake::new(hash_clone, [0; 20]))
                                    .await;
                            }));
                        }
                    }
                }

                for handle in handles {
                    handle.await.expect("Task panicked");
                }

                dbg!();
            }
            _ => println!(),
        },
        _ => println!("Invalid torrent file!"),
    }
}
