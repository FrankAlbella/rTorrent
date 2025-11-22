use std::{fs, io, path::PathBuf, sync::Arc, sync::Mutex};

use bytes::{Bytes, BytesMut};

use crate::{
    bencode::{self, BencodeParseErr, BencodeType},
    handshake::Handshake,
    meta_info::{FromBencodeTypeErr, FromBencodemap, MetaInfo},
    peer::Peer,
    tracker,
};

#[derive(Debug, Clone)]
pub struct Torrent {
    meta_info: Arc<MetaInfo>,
    connected_peers: Arc<Mutex<Vec<Peer>>>,
}

#[derive(Debug, thiserror::Error)]
pub enum TorrentErr {
    #[error("Failed to read torrent file")]
    IoErr(#[from] io::Error),
    #[error("Failed to decode file")]
    BencodeParseErr(#[from] BencodeParseErr),
    #[error("Failed to extract meta info from file")]
    FromBencodeTypeErr(#[from] FromBencodeTypeErr),
    #[error("Invalid torrent file")]
    InvalidFile(PathBuf),
}

impl Torrent {
    pub fn new(meta_info: MetaInfo) -> Self {
        Torrent {
            meta_info: Arc::new(meta_info),
            connected_peers: Arc::new(Mutex::new(Vec::new())),
        }
    }

    pub async fn start(self: &Self) {
        self.connect_to_peers().await;
        //self.send_bitfield_to_peers().await;
    }

    pub fn get_bitfield(&self) -> Bytes {
        let total_length = self.meta_info.info.length.unwrap();
        let piece_length = self.meta_info.info.piece_length;
        let num_pieces = (total_length + piece_length - 1) / piece_length;

        let num_bytes = (num_pieces + 7) / 8;

        let mut buf_bitfield = BytesMut::new();
        buf_bitfield.resize(num_bytes as usize, 0);
        buf_bitfield.freeze()
    }

    pub async fn send_bitfield_to_peers(&self) {
        let buf_bitfield = self.get_bitfield();
        let peers_vec_clone = self.connected_peers.clone();

        for peer in peers_vec_clone.lock().unwrap().iter_mut() {
            let result = peer.send_bitfield(&buf_bitfield).await;
            dbg!("{:#?}", result);
        }
    }

    pub fn from_file(path: &PathBuf) -> Result<Self, TorrentErr> {
        let contents = fs::read(path)?;

        let bencode_vec = bencode::decode_to_vec(&contents)?;

        if let Some(first) = bencode_vec.first() {
            match first {
                BencodeType::Dictionary(map) => {
                    let data = MetaInfo::from_bencodemap(map)?;
                    return Ok(Torrent::new(data));
                }
                _ => Err(TorrentErr::InvalidFile(path.clone())),
            }
        } else {
            Err(TorrentErr::InvalidFile(path.clone()))
        }
    }

    pub async fn connect_to_peers(self: &Self) {
        let response = tracker::send_get_request(&self.meta_info).await;
        let mut handles = Vec::new();

        if let Ok(res) = response {
            if let Some(peers_vec) = res.peers {
                for mut peer in peers_vec {
                    let hash_clone = self.meta_info.hash.clone();
                    let peers_vec_clone = self.connected_peers.clone();
                    let bitfield = self.get_bitfield();
                    let piece_length = self.meta_info.info.piece_length;
                    let piece_hash = self.meta_info.info.get_piece_hash(0).unwrap();
                    handles.push(tokio::spawn(async move {
                        match peer.connect(&Handshake::new(hash_clone, [0; 20])).await {
                            Ok(_) => {
                                let mut peers = peers_vec_clone.lock().unwrap();
                                //peers.push(peer);
                                println!("Connected to peer");
                            }
                            Err(e) => {
                                println!("Failed to connect to peer: {:#?}", e);
                                return;
                            }
                        }

                        match peer.send_bitfield(&bitfield).await {
                            Ok(result) => println!("Bitfield received!"),
                            Err(e) => {
                                println!("Failed to send bitfield: {:#?}", e);
                                return;
                            }
                        }

                        match peer
                            .download_piece(0, piece_length as u64, piece_hash)
                            .await
                        {
                            Ok(result) => println!("Downloaded piece: {:#?}", result),
                            Err(e) => println!("Failed to download piece: {:#?}", e),
                        }
                    }));
                }
            }
        }

        for handle in handles {
            handle.await.expect("Task panicked");
        }
    }

    pub fn from_magnet(_magnet: &str) -> Result<Self, TorrentErr> {
        todo!("Add support for magnet strings")
    }
}
