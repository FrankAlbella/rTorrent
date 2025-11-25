const PROTOCOL: &[u8; PROTOCOL_SIZE] = b"BitTorrent protocol";

const LEGNTH_SIZE: usize = 1;
const PROTOCOL_SIZE: usize = 19;
const RESERVED_SIZE: usize = 8;
const INFOHASH_SIZE: usize = 20;
const PEER_ID_SIZE: usize = 20;
pub const TOTAL_SIZE: usize =
    LEGNTH_SIZE + PROTOCOL_SIZE + RESERVED_SIZE + INFOHASH_SIZE + PEER_ID_SIZE;

const LEGNTH_OFFSET: usize = 0;
const PROTOCOL_OFFSET: usize = LEGNTH_OFFSET + LEGNTH_SIZE;
const RESERVED_OFFSET: usize = PROTOCOL_OFFSET + PROTOCOL_SIZE;
const INFOHASH_OFFSET: usize = RESERVED_OFFSET + RESERVED_SIZE;
const PEER_ID_OFFSET: usize = INFOHASH_OFFSET + INFOHASH_SIZE;

#[derive(Debug, Clone, PartialEq)]
pub struct Handshake {
    pub length: u8,
    pub protocol: [u8; PROTOCOL_SIZE],
    pub reserved: [u8; RESERVED_SIZE],
    pub info_hash: [u8; INFOHASH_SIZE],
    pub peer_id: [u8; PEER_ID_SIZE],
}

#[derive(Debug, PartialEq)]
pub enum HandshakeErr {
    InvalidSize,
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Handshake {
            length: PROTOCOL_SIZE.try_into().unwrap(),
            protocol: *PROTOCOL,
            reserved: [0; RESERVED_SIZE],
            info_hash: info_hash,
            peer_id: peer_id,
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Self, HandshakeErr> {
        if bytes.len() != TOTAL_SIZE {
            return Err(HandshakeErr::InvalidSize);
        }

        Ok(Handshake {
            length: bytes[LEGNTH_OFFSET],
            protocol: bytes[PROTOCOL_OFFSET..RESERVED_OFFSET].try_into().unwrap(),
            reserved: bytes[RESERVED_OFFSET..INFOHASH_OFFSET].try_into().unwrap(),
            info_hash: bytes[INFOHASH_OFFSET..PEER_ID_OFFSET].try_into().unwrap(),
            peer_id: bytes[PEER_ID_OFFSET..TOTAL_SIZE].try_into().unwrap(),
        })
    }

    pub fn to_bytes(self: &Self) -> [u8; TOTAL_SIZE] {
        let mut result: [u8; TOTAL_SIZE] = [0; TOTAL_SIZE];
        result[LEGNTH_OFFSET] = self.length;
        result[PROTOCOL_OFFSET..RESERVED_OFFSET].copy_from_slice(&self.protocol);
        result[RESERVED_OFFSET..INFOHASH_OFFSET].copy_from_slice(&self.reserved);
        result[INFOHASH_OFFSET..PEER_ID_OFFSET].copy_from_slice(&self.info_hash);
        result[PEER_ID_OFFSET..TOTAL_SIZE].copy_from_slice(&self.peer_id);
        result
    }

    pub fn is_valid(self: &Self, other: &Handshake) -> bool {
        self.length == other.length
            && self.protocol == other.protocol
            && self.info_hash == other.info_hash
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn conversion() {
        let info_hash = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        ];

        let peer_id = [
            1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16, 17, 18, 19, 20,
        ];
        let hs = Handshake::new(info_hash, peer_id);
        let ab = hs.to_bytes();
        let hs2 = Handshake::from_bytes(&ab.to_vec()).unwrap();

        assert_eq!(hs, hs2);
    }
}
