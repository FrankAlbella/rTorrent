use std::{fs, io, path::PathBuf, sync::Arc, sync::Mutex};

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
                for peer in peers_vec {
                    let hash_clone = self.meta_info.hash;
                    let peer_clone = peer;
                    handles.push(tokio::spawn(async move {
                        match peer_clone
                            .connect(&Handshake::new(hash_clone, [0; 20]))
                            .await
                        {
                            Ok(_) => {
                                //let mut peers = self.connected_peers.lock();
                                //self.connected_peers.lock().push(peer_clone);
                                println!("Connected to peer");
                            }
                            Err(e) => println!("Failed to connect to peer: {:#?}", e),
                        }
                    }));
                }
            }
        }

        for handle in handles {
            handle.await.expect("Task panicked");
        }
    }

    pub fn from_magnet(magnet: &str) -> Result<Self, TorrentErr> {
        todo!("Add support for magnet strings")
    }
}
