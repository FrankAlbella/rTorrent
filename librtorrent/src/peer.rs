use std::sync::Arc;

use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, ErrorKind, Interest},
    net::TcpStream,
};

use crate::{
    bencode::{BencodeMap, BencodeMapDecoder},
    handshake::Handshake,
    message::{Message, MessageErr, MessageType},
    meta_info::{FromBencodeTypeErr, FromBencodemap},
    piece_manager::PieceManager,
};

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
pub enum PeerEvent {
    Connected,
    Disconnected,
    HandshakeReceived(Handshake),
    HandshakeSent(Handshake),
    MessageReceived(Message),
    MessageSent(Message),
}

#[derive(Debug, Clone)]
pub enum PeerState {
    Disconnected,
    Choked,
    Interested,
    Downloading,
    Idle,
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
    #[error("Tokio write error: {0}")]
    TokioWriteError(std::io::Error),
    #[error("Tokio read error: {0}")]
    TokioReadError(std::io::Error),
    #[error("Tokio connect error: {0}")]
    TokioConnectError(std::io::Error),
    #[error("Invalid connection")]
    InvalidConnection,
    #[error("Invalid handshake")]
    InvalidHandshake,
    #[error("Invalid message {0}")]
    InvalidMessage(#[from] MessageErr),
    #[error("Unexpected message: {0}")]
    UnexpectedMessage(String),
    #[error("Unexpected IO error {0}")]
    UnexpectedIoError(#[from] std::io::Error),
}

impl Peer {
    pub fn new(peer_id: Option<String>, ip: String, port: i64) -> Self {
        Peer {
            peer_id,
            ip,
            port,
            socket: None,
            my_state: PeerState::Disconnected,
            their_state: PeerState::Disconnected,
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

    pub async fn start(
        &mut self,
        piece_manager: &PieceManager,
        torrent_hash: Arc<[u8; 20]>,
    ) -> Result<(), ConnectionErr> {
        self.connect(&Handshake::new(*torrent_hash, [0u8; 20]))
            .await?;
        self.log("Connected to peer");

        let bitfield = piece_manager.get_bitfield();

        self.log("Sending bitfield!");
        let their_bitfield = self.send_bitfield(&bitfield).await?;
        self.log("Bitfield received!");

        let piece_length = piece_manager.get_piece_length();

        while let Some(index) = piece_manager.get_next_piece(&their_bitfield) {
            self.log(&format!("Attempting to download piece {index}"));
            if matches!(self.my_state, PeerState::Choked) {
                self.log("Peer is chocking us, sending interested");
                self.send_interested().await?;

                self.my_state = PeerState::Interested;
            }

            let result = self.download_piece(index, piece_length as u64).await?;
            if piece_manager.add_piece(&index, result) {
                self.log(&format!(
                    "Piece {index} successfully downloaded and verified!"
                ));
            } else {
                self.log(&format!("Piece {index} download failed!"));
            }
        }

        Ok(())
    }

    pub async fn send_interested(&mut self) -> Result<(), ConnectionErr> {
        let message = Message::new(1, Some(MessageType::Interested as u8), None);

        self.log("Sending interested message");

        let res = self.send_message(&message).await?;

        if res.id != Some(MessageType::Unchoke as u8) {
            return Err(ConnectionErr::UnexpectedMessage(
                "Expected unchoke message".to_string(),
            ));
        }

        self.log("Recieved unchoke message");

        Ok(())
    }

    pub async fn download_piece(
        &mut self,
        piece_index: usize,
        piece_length: u64,
    ) -> Result<Bytes, ConnectionErr> {
        // Send request for piece
        const MAX_BLOCK_SIZE: usize = (2 as usize).pow(14);
        let num_blocks = (piece_length as usize + MAX_BLOCK_SIZE - 1) / MAX_BLOCK_SIZE;
        let mut piece_buffer = BytesMut::with_capacity(piece_length as usize);
        let mut remaining = piece_length as usize;

        self.log(&format!(
            "Downloading piece {piece_index} with {num_blocks} blocks"
        ));
        for block_index in 0..num_blocks {
            let offset = block_index * MAX_BLOCK_SIZE;
            let block_size = MAX_BLOCK_SIZE.min(remaining);
            remaining = remaining - block_size;

            let mut buf = BytesMut::with_capacity(12);
            buf.put_u32(piece_index as u32); // index
            buf.put_u32(offset as u32); // begin
            buf.put_u32(block_size as u32); // length

            let message = Message {
                length: 13,
                id: Some(MessageType::Request as u8),
                payload: Some(buf.freeze()),
            };

            self.log("Sending request message");

            let res = self.send_message(&message).await?;

            if res.id != Some(MessageType::Piece as u8) {
                return Err(ConnectionErr::UnexpectedMessage(
                    "Expected piece message".to_string(),
                ));
            }

            if let Some(payload) = res.payload {
                // Skip the first 8 bytes (piece index and offset)
                piece_buffer.extend_from_slice(&payload[8..]);
            }

            self.log(&format!(
                "Block {block_index} of {num_blocks} for piece {piece_index} recieved"
            ));
        }

        self.log(&format!("Piece {piece_index} recieved"));

        Ok(piece_buffer.freeze())
    }

    pub async fn send_bitfield(&mut self, bitfield: &Bytes) -> Result<Bytes, ConnectionErr> {
        let msg = Message {
            length: (bitfield.len() + 1) as u32,
            id: Some(MessageType::Bitfield as u8),
            payload: Some(bitfield.clone()),
        };

        let result = self.send_message(&msg).await;

        match result {
            Ok(msg) if msg.id == Some(MessageType::Bitfield as u8) => {
                if let Some(payload) = msg.payload {
                    Ok(payload)
                } else {
                    Err(ConnectionErr::UnexpectedMessage(
                        "Expected Bitfield message payload to be Some".to_string(),
                    ))
                }
            }
            Err(e) => Err(e),
            _ => Err(ConnectionErr::UnexpectedMessage(
                "Expected Bitfield message".to_string(),
            )),
        }
    }

    /// Establishes a connection and performs handshake with peer
    pub async fn connect(self: &mut Self, handshake: &Handshake) -> Result<(), ConnectionErr> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.ip, self.port))
            .await
            .map_err(|e| ConnectionErr::TokioConnectError(e))?;

        stream.write_all(&handshake.to_bytes()).await?;

        let mut buf: [u8; crate::handshake::TOTAL_SIZE] = [0; crate::handshake::TOTAL_SIZE];
        stream.read_exact(&mut buf).await?;

        if let Ok(hs) = Handshake::from_bytes(&buf) {
            if hs.is_valid(&handshake) {
                self.socket = Some(Box::new(stream));
                self.my_state = PeerState::Choked;
                self.their_state = PeerState::Choked;
                return Ok(());
            }
        }

        Err(ConnectionErr::InvalidHandshake)
    }

    async fn send_message(&mut self, message: &Message) -> Result<Message, ConnectionErr> {
        let stream = match self.socket.as_mut() {
            Some(stream) => stream.as_mut(),
            None => return Err(ConnectionErr::InvalidConnection),
        };

        stream.write_all(&message.to_bytes()).await?;

        Ok(Message::from_stream(stream).await?)
    }

    fn log(&self, message: &str) {
        println!("Peer @ {}:{}:\t{}", self.ip, self.port, message);
    }
}
