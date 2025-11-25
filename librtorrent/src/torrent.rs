use std::{fs, io, path::PathBuf, sync::Arc};

use crate::{
    bencode::{self, BencodeParseErr, BencodeType},
    meta_info::{FromBencodeTypeErr, FromBencodemap, MetaInfo},
    peer_manager::PeerManager,
};

#[derive(Debug)]
pub struct Torrent {
    meta_info: Arc<MetaInfo>,
    peer_manager: PeerManager,
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
    pub async fn new(meta_info: MetaInfo) -> Self {
        let arc = Arc::new(meta_info);
        Torrent {
            meta_info: arc.clone(),
            peer_manager: PeerManager::new(arc.clone()).await,
        }
    }

    pub async fn start(&mut self) {
        let result = self.peer_manager.start().await;
        println!("Torrent started {result:#?}");
    }

    pub async fn from_file(path: &PathBuf) -> Result<Self, TorrentErr> {
        let contents = fs::read(path)?;

        let bencode_vec = bencode::decode_to_vec(&contents)?;

        if let Some(first) = bencode_vec.first() {
            match first {
                BencodeType::Dictionary(map) => {
                    let data = MetaInfo::from_bencodemap(map)?;
                    return Ok(Torrent::new(data).await);
                }
                _ => Err(TorrentErr::InvalidFile(path.clone())),
            }
        } else {
            Err(TorrentErr::InvalidFile(path.clone()))
        }
    }

    pub fn from_magnet(_magnet: &str) -> Result<Self, TorrentErr> {
        todo!("Add support for magnet strings")
    }
}
