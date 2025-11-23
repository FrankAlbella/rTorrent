use bytes::{Bytes, BytesMut};
use sha1::{Digest, Sha1};

use crate::meta_info::MetaInfo;

struct PieceManager {
    bitfield: Bytes,
    piece_hashes: Vec<[u8; 20]>,
}

impl PieceManager {
    pub fn new(meta_info: &MetaInfo) -> Self {
        PieceManager {
            bitfield: Self::meta_info_to_bitfield(meta_info),
            piece_hashes: meta_info.info.get_piece_hashes(),
        }
    }

    fn meta_info_to_bitfield(meta_info: &MetaInfo) -> Bytes {
        let total_length = match meta_info.info.length {
            Some(length) => length,
            None => todo!("Mutli-file torrents are not yet supported!"),
        };
        let piece_length = meta_info.info.piece_length;
        let num_pieces = (total_length + piece_length - 1) / piece_length;

        let num_bytes = (num_pieces + 7) / 8;

        let mut buf_bitfield = BytesMut::new();
        buf_bitfield.resize(num_bytes as usize, 0);

        buf_bitfield.freeze()
    }

    pub fn is_piece_valid(&self, piece_index: usize, piece: &Bytes) -> bool {
        let downloaded_hash: [u8; 20] = Sha1::digest(&piece).into();

        if let Some(hash) = self.piece_hashes.get(piece_index) {
            downloaded_hash == *hash
        } else {
            false
        }
    }

    pub fn get_bitfield(&self) -> &Bytes {
        &self.bitfield
    }
}
