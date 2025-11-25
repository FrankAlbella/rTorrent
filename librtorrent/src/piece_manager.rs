use std::{collections::HashMap, sync::Mutex, sync::RwLock};

use bytes::{Bytes, BytesMut};
use sha1::{Digest, Sha1};

use crate::meta_info::MetaInfo;

#[derive(Debug)]
pub struct PieceManager {
    bitfield: RwLock<BytesMut>,
    piece_hashes: Vec<[u8; 20]>,
    piece_length: usize,
    torrent_hash: [u8; 20],
    piece_map: Mutex<HashMap<usize, PieceStatus>>,
}

#[derive(Debug)]
enum PieceStatus {
    NotStarted,
    InProgress,
    Completed(Bytes),
}

impl PieceManager {
    pub fn new(meta_info: &MetaInfo) -> Self {
        PieceManager {
            bitfield: RwLock::new(Self::meta_info_to_bitfield(meta_info)),
            piece_hashes: meta_info.info.get_piece_hashes(),
            piece_length: meta_info.info.piece_length as usize,
            torrent_hash: meta_info.hash.clone(),
            piece_map: Mutex::new(HashMap::new()),
        }
    }

    fn meta_info_to_bitfield(meta_info: &MetaInfo) -> BytesMut {
        let total_length = match meta_info.info.length {
            Some(length) => length,
            None => todo!("Mutli-file torrents are not yet supported!"),
        };
        let piece_length = meta_info.info.piece_length;
        let num_pieces = (total_length + piece_length - 1) / piece_length;

        let num_bytes = (num_pieces + 7) / 8;

        // TODO: actually load already downloaded pieces into bitfield
        let mut buf_bitfield = BytesMut::new();
        buf_bitfield.resize(num_bytes as usize, 0);
        buf_bitfield
    }

    pub fn is_piece_valid(&self, piece_index: &usize, piece: &Bytes) -> bool {
        let downloaded_hash: [u8; 20] = Sha1::digest(&piece).into();

        if let Some(hash) = self.piece_hashes.get(*piece_index) {
            downloaded_hash == *hash
        } else {
            false
        }
    }

    // TODO: improve performance; cloning bitfield: BytesMut is expensive
    pub fn get_bitfield(&self) -> Bytes {
        self.bitfield.read().unwrap().clone().freeze()
    }

    /// Return the index of the piece we need from a peer.
    /// If peer has no pieces we need then we return None.
    pub fn get_next_piece(&self, their_bitfield: &Bytes) -> Option<usize> {
        for (index, (&my_byte, &their_byte)) in self
            .bitfield
            .read()
            .unwrap()
            .iter()
            .zip(their_bitfield.iter())
            .enumerate()
        {
            // Get only the bits we don't have and they have set as 1
            let diff = (!my_byte) & their_byte;

            // If there are no differences, continue
            if diff == 0 {
                continue;
            }

            // Iterate over the bits in the diff byte
            for bit_index in 0..8 {
                let mask = 1 << (7 - bit_index);

                if diff & mask != 0 {
                    // Calculate the piece index from the bit index
                    let piece_index = index * 8 + bit_index;
                    let mut map = self.piece_map.lock().unwrap();
                    match map.get(&piece_index) {
                        Some(PieceStatus::InProgress) => continue,
                        Some(PieceStatus::Completed(_)) => continue,
                        _ => {
                            map.insert(piece_index, PieceStatus::InProgress);
                            return Some(piece_index);
                        }
                    }
                }
            }
        }

        None
    }

    pub fn get_piece_length(&self) -> usize {
        self.piece_length
    }

    pub fn get_torrent_hash(&self) -> &[u8; 20] {
        &self.torrent_hash
    }

    /// Verify piece hash and, if valid, store it and update local bitfield
    /// Returns true if the piece was successfully added, false otherwise.
    pub fn add_piece(&self, index: &usize, bytes: Bytes) -> bool {
        if self.is_piece_valid(&index, &bytes) {
            {
                let mut map = self.piece_map.lock().unwrap();
                map.insert(*index, PieceStatus::Completed(bytes));
            }

            self.update_bitfield(index);
            true
        } else {
            let mut map = self.piece_map.lock().unwrap();
            map.insert(*index, PieceStatus::NotStarted);
            false
        }
    }

    pub fn cancel_piece(&mut self, index: &usize) {
        let mut map = self.piece_map.lock().unwrap();

        // We only need to update if the piece is in progress
        map.entry(*index)
            .and_modify(|status| {
                if !matches!(status, PieceStatus::Completed(_)) {
                    *status = PieceStatus::NotStarted;
                }
            })
            .or_insert(PieceStatus::NotStarted);
    }

    fn update_bitfield(&self, index: &usize) {
        let byte_index = index / 8;
        let bit_index = index % 8;
        let mask = 1 << (7 - bit_index);

        let mut bitfield = self.bitfield.write().unwrap();
        bitfield[byte_index] |= mask;
    }

    pub fn save_to_disk(&self) {
        todo!()
    }
}

#[cfg(test)]
mod tests {
    use crate::meta_info::TorrentInfo;

    use super::*;

    #[test]
    fn test_get_next_piece_index_0() {
        let meta_info = MetaInfo {
            announce: Some("test".to_string()),
            nodes: None,
            url_list: None,
            announce_list: None,
            hash: [0u8; 20],
            info: TorrentInfo {
                name: "test".to_string(),
                piece_length: 2 << 14,
                pieces: vec![],
                length: Some(8),
                files: None,
                private: None,
            },
        };
        let piece_manager = PieceManager::new(&meta_info);
        let bitfield = Bytes::from(vec![0b10000000]);
        assert_eq!(piece_manager.get_next_piece(&bitfield), Some(0));
    }

    #[test]
    fn test_get_next_piece_index_7() {
        let meta_info = MetaInfo {
            announce: Some("test".to_string()),
            nodes: None,
            url_list: None,
            announce_list: None,
            hash: [0u8; 20],
            info: TorrentInfo {
                name: "test".to_string(),
                piece_length: 2 << 14,
                pieces: vec![],
                length: Some(8),
                files: None,
                private: None,
            },
        };
        let piece_manager = PieceManager::new(&meta_info);
        let bitfield = Bytes::from(vec![0b00000001]);
        assert_eq!(piece_manager.get_next_piece(&bitfield), Some(7));
    }

    #[test]
    fn test_get_next_piece_mutliple_pieces() {
        let meta_info = MetaInfo {
            announce: Some("test".to_string()),
            nodes: None,
            url_list: None,
            announce_list: None,
            hash: [0u8; 20],
            info: TorrentInfo {
                name: "test".to_string(),
                piece_length: 2 << 14,
                pieces: vec![],
                length: Some(8),
                files: None,
                private: None,
            },
        };
        let piece_manager = PieceManager::new(&meta_info);
        let bitfield = Bytes::from(vec![0b00000011]);
        assert_eq!(piece_manager.get_next_piece(&bitfield), Some(6));
        assert_eq!(piece_manager.get_next_piece(&bitfield), Some(7));
    }
}
