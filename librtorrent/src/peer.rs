use bytes::Bytes;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::io::ErrorKind;
use tokio::io::Interest;
use tokio::net::TcpStream;

use crate::bencode::BencodeMap;
use crate::bencode::BencodeMapDecoder;
use crate::handshake::Handshake;
use crate::message::{Message, MessageErr};
use crate::meta_info::FromBencodeTypeErr;
use crate::meta_info::FromBencodemap;

// Peer keys
const PEER_ID_KEY: &str = "peer id";
const IP_KEY: &str = "ip";
const PORT_KEY: &str = "port";

#[derive(Debug)]
pub struct Peer {
    pub peer_id: Option<String>,
    pub ip: String,
    pub port: i64,
    pub socket: Option<Box<TcpStream>>,
    pub my_state: PeerState,
    pub their_state: PeerState,
}

#[derive(Debug, Clone)]
pub struct PeerState {
    choked: bool,
    interested: bool,
    bitfield: Vec<bool>,
}

impl PeerState {
    pub fn new() -> Self {
        PeerState {
            choked: true,
            interested: false,
            bitfield: Vec::new(),
        }
    }
}

impl FromBencodemap for Peer {
    fn from_bencodemap(bencode_map: &BencodeMap) -> Result<Self, FromBencodeTypeErr> {
        if !Self::is_valid_bencodemap(bencode_map) {
            return Err(FromBencodeTypeErr::MissingValue(String::from(
                "Missing values for peer",
            )));
        }

        // Safe unwraps because we checked the values exist in the map above
        let peer_id: Option<String> = bencode_map.get_decode(PEER_ID_KEY);
        let ip: String = bencode_map.get_decode(IP_KEY).unwrap();
        let port: i64 = bencode_map.get_decode(PORT_KEY).unwrap();

        Ok(Peer::new(peer_id, ip, port))
    }

    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool {
        bencode_map.contains_key(IP_KEY.as_bytes()) && bencode_map.contains_key(PORT_KEY.as_bytes())
    }
}

#[derive(Debug, Error)]
pub enum ConnectionErr {
    #[error("Tokio error: {0}")]
    TokioError(std::io::Error),
    #[error("Invalid connection")]
    InvalidConnection,
    #[error("Invalid handshake")]
    InvalidHandshake,
    #[error("Invalid message")]
    InvalidMessage(#[from] MessageErr),
}

impl Peer {
    pub fn new(peer_id: Option<String>, ip: String, port: i64) -> Self {
        Peer {
            peer_id,
            ip,
            port,
            socket: None,
            my_state: PeerState::new(),
            their_state: PeerState::new(),
        }
    }

    pub fn from_bencodemap_list(
        bencode_map: &Vec<BencodeMap>,
    ) -> Result<Vec<Self>, FromBencodeTypeErr> {
        bencode_map
            .iter()
            .map(|x| Peer::from_bencodemap(x))
            .collect()
    }

    pub async fn download_piece(
        &mut self,
        piece_index: usize,
        piece_length: u64,
    ) -> Result<(), ConnectionErr> {
        if self.socket.is_none() {
            return Err(ConnectionErr::InvalidConnection);
        }

        Ok(())
    }

    pub async fn send_bitfield(&mut self, bitfield: &Bytes) -> Result<Message, ConnectionErr> {
        let msg = Message {
            length: (bitfield.len() + 1) as u32,
            id: 5,
            payload: Some(bitfield.clone()),
        };

        self.send_message(&msg)
            .await
            .map_err(|_| ConnectionErr::InvalidConnection)
    }

    /// Establishes a connection and performs handshake with peer
    pub async fn connect(self: &mut Self, handshake: &Handshake) -> Result<(), ConnectionErr> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.ip, self.port))
            .await
            .map_err(|e| ConnectionErr::TokioError(e))?;

        loop {
            let ready = stream.ready(Interest::WRITABLE).await.unwrap();

            if ready.is_writable() {
                match stream.write_all(&handshake.to_bytes()).await {
                    Ok(_) => {
                        break;
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(ConnectionErr::TokioError(e));
                    }
                }
            }
        }

        loop {
            let ready = stream.ready(Interest::READABLE).await.unwrap();

            if ready.is_readable() {
                let mut buf: [u8; crate::handshake::TOTAL_SIZE] = [0; crate::handshake::TOTAL_SIZE];
                match stream.try_read(&mut buf) {
                    Ok(_) => match Handshake::from_bytes(&buf.to_vec()) {
                        Ok(their_hs) if their_hs.is_valid(&handshake) => {
                            self.socket = Some(Box::new(stream));
                            return Ok(());
                        }
                        _ => return Err(ConnectionErr::InvalidHandshake),
                    },
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(ConnectionErr::TokioError(e));
                    }
                };
            }
        }
    }

    async fn send_message(&mut self, message: &Message) -> Result<Message, ConnectionErr> {
        if self.socket.is_none() {
            return Err(ConnectionErr::InvalidConnection);
        }

        let stream = self.socket.as_mut().unwrap();

        loop {
            let ready = stream.ready(Interest::WRITABLE).await.unwrap();

            if ready.is_writable() {
                match stream.write_all(&message.to_bytes()).await {
                    Ok(_) => {
                        break;
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(ConnectionErr::TokioError(e));
                    }
                }
            }
        }

        loop {
            let ready = stream.ready(Interest::READABLE).await.unwrap();

            if ready.is_readable() {
                let mut buf: [u8; 100000] = [0; 100000];
                match stream.try_read(&mut buf) {
                    Ok(_) => return Ok(Message::from_bytes(&buf)?),
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(ConnectionErr::TokioError(e));
                    }
                };
            }
        }
    }
}
