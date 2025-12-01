use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use thiserror::Error;
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

use crate::{
    bencode::{BencodeMap, BencodeMapDecoder},
    handshake::Handshake,
    message::{Message, MessageErr},
    meta_info::FromBencodeTypeErr,
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
    pub socket: Option<TcpStream>,
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

impl TryFrom<&BencodeMap> for Peer {
    type Error = FromBencodeTypeErr;

    fn try_from(bencode_map: &BencodeMap) -> Result<Self, Self::Error> {
        let peer_id: Option<String> = bencode_map.get_decode(PEER_ID_KEY);
        let ip: String = bencode_map
            .get_decode(IP_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(IP_KEY.to_string()))?;
        let port: i64 = bencode_map
            .get_decode(PORT_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(IP_KEY.to_string()))?;

        Ok(Peer::new(peer_id, ip, port))
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
        bencode_map: &[BencodeMap],
    ) -> Result<Vec<Self>, FromBencodeTypeErr> {
        bencode_map.iter().map(Peer::try_from).collect()
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
            if piece_manager.add_piece(&index, result).await {
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
        let message = Message::Interested;

        self.log("Sending interested message");

        let res = self.send_message(&message).await?;

        if !matches!(res, Message::Unchoke) {
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
        const MAX_BLOCK_SIZE: usize = 1 << 14; // 16KB
        let num_blocks = (piece_length as usize).div_ceil(MAX_BLOCK_SIZE);
        let mut piece_buffer = BytesMut::with_capacity(piece_length as usize);
        let mut remaining = piece_length as usize;

        self.log(&format!(
            "Downloading piece {piece_index} with {num_blocks} blocks"
        ));
        for block_index in 0..num_blocks {
            let offset = block_index * MAX_BLOCK_SIZE;
            let block_size = MAX_BLOCK_SIZE.min(remaining);
            remaining -= block_size;

            let message = Message::Request {
                index: piece_index as u32,
                begin: offset as u32,
                length: block_size as u32,
            };

            self.log("Sending request message");

            let res = self.send_message(&message).await?;

            match res {
                Message::Piece {
                    index: _,
                    begin: _,
                    block,
                } => {
                    piece_buffer.extend_from_slice(&block);
                }
                _ => {
                    return Err(ConnectionErr::UnexpectedMessage(
                        "Expected piece message".to_string(),
                    ));
                }
            }

            self.log(&format!(
                "Block {block_index} of {num_blocks} for piece {piece_index} recieved"
            ));
        }

        self.log(&format!("Piece {piece_index} recieved"));

        Ok(piece_buffer.freeze())
    }

    pub async fn send_bitfield(&mut self, bitfield: &Bytes) -> Result<Bytes, ConnectionErr> {
        let msg = Message::Bitfield {
            bitfield: bitfield.clone(),
        };

        let result = self.send_message(&msg).await;

        match result {
            Ok(Message::Bitfield { bitfield: payload }) => Ok(payload),
            Err(e) => Err(e),
            _ => Err(ConnectionErr::UnexpectedMessage(
                "Expected Bitfield message".to_string(),
            )),
        }
    }

    /// Establishes a connection and performs handshake with peer
    pub async fn connect(&mut self, handshake: &Handshake) -> Result<(), ConnectionErr> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.ip, self.port))
            .await
            .map_err(ConnectionErr::TokioConnectError)?;

        stream.write_all(&handshake.to_bytes()).await?;

        let mut buf: [u8; crate::handshake::TOTAL_SIZE] = [0; crate::handshake::TOTAL_SIZE];
        stream.read_exact(&mut buf).await?;

        if let Ok(hs) = Handshake::from_bytes(&buf) {
            if hs.is_valid(handshake) {
                self.socket = Some(stream);
                self.my_state = PeerState::Choked;
                self.their_state = PeerState::Choked;
                return Ok(());
            }
        }

        Err(ConnectionErr::InvalidHandshake)
    }

    async fn send_message(&mut self, message: &Message) -> Result<Message, ConnectionErr> {
        let stream = match self.socket.as_mut() {
            Some(stream) => stream,
            None => return Err(ConnectionErr::InvalidConnection),
        };

        stream.write_all(&message.to_bytes()).await?;

        Ok(Message::from_stream(stream).await?)
    }

    fn log(&self, message: &str) {
        println!("Peer @ {}:{}:\t{}", self.ip, self.port, message);
    }
}
