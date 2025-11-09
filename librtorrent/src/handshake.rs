const HEADER: &str = "BitTorrent protocol";

pub struct Handshake {
    length: u8,
    header: [u8; 19],
    reserved: [u8; 8],
    info_hash: [u8; 20],
    peer_id: [u8; 20],
}

impl Handshake {
    pub fn new(info_hash: [u8; 20], peer_id: [u8; 20]) -> Self {
        Handshake {
            length: 19,
            header: [0; 19],
            reserved: [0; 8],
            info_hash: info_hash,
            peer_id: peer_id,
        }
    }
}
