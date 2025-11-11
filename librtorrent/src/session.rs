use std::path::PathBuf;

use log::warn;

use crate::torrent::Torrent;

pub struct Session {
    // TODO: this could be a HashMap with info_hash as key
    torrents: Vec<Torrent>,
}

impl Session {
    pub fn new() -> Self {
        Self {
            torrents: Vec::new(),
        }
    }

    pub async fn start(self: &Self) {
        for torrent in &self.torrents {
            torrent.start().await;
        }
    }

    pub fn add_torrent(self: &mut Self, path: &str) {
        if Session::is_torrent_file(path) {
            match Torrent::from_file(&PathBuf::from(path)) {
                Ok(torrent) => self.torrents.push(torrent),
                Err(error) => warn!("Failed to add torrent with error: {error:#?}"),
            }
        } else {
            match Torrent::from_magnet(path) {
                Ok(torrent) => self.torrents.push(torrent),
                Err(error) => warn!("Failed to add torrent with error: {error:#?}"),
            }
        }
    }

    //TODO: better way to check if input is file or magnet
    fn is_torrent_file(path: &str) -> bool {
        path.ends_with(".torrent")
    }
}
