use bytes::BufMut;
use bytes::Bytes;
use bytes::BytesMut;
use thiserror::Error;
use tokio::io::AsyncWriteExt;
use tokio::io::ErrorKind;
use tokio::io::Interest;
use tokio::net::TcpStream;

use crate::bencode::BencodeMap;
use crate::bencode::BencodeMapDecoder;
use crate::handshake::Handshake;
use crate::message::{Message, MessageErr, MessageType};
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
    #[error("Invalid message")]
    InvalidMessage(#[from] MessageErr),
    #[error("Unexpected message: {0}")]
    UnexpectedMessage(String),
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
        piece_index: u8,
        piece_length: u64,
    ) -> Result<(), ConnectionErr> {
        if self.socket.is_none() {
            return Err(ConnectionErr::InvalidConnection);
        }

        //let payload = Bytes::from(&piece_index.to_be_bytes());
        let message = Message::new(1, Some(MessageType::Interested as u8), None);

        let res = self.send_message(&message).await?;

        println!("Sent interested message");

        if res.id != Some(MessageType::Unchoke as u8) {
            self.wait_for_message(MessageType::Unchoke as u8).await?;
        }

        println!("Recieved unchoke message");

        // Send request for piece
        const BLOCK_SIZE: usize = (2 as usize).pow(14);
        let num_blocks = (piece_length as usize + BLOCK_SIZE - 1) / BLOCK_SIZE;
        let block_size = BLOCK_SIZE.min(piece_length as usize);
        let mut piece_buffer = BytesMut::with_capacity(piece_length as usize);

        let mut buf = BytesMut::with_capacity(12);
        buf.put_u32(piece_index as u32); // index
        buf.put_u32(0); // begin
        buf.put_u32(block_size as u32); // length

        let message = Message {
            length: 13,
            id: Some(MessageType::Request as u8),
            payload: Some(buf.freeze()),
        };

        println!("Sending request message: {message:#?}");

        let res = self.send_message(&message).await?;
        print!("Message received {res:#?}");
        if res.id != Some(MessageType::Piece as u8) {
            self.wait_for_message(MessageType::Piece as u8).await?;
        }
        print!("Piece recieved {res:#?}");

        Ok(())
    }

    // Read from socket until we get a message with the specified id
    pub async fn wait_for_message(&mut self, id: u8) -> Result<Message, ConnectionErr> {
        let stream = self.socket.as_mut().unwrap();
        loop {
            let ready = stream.ready(Interest::READABLE).await.unwrap();

            if ready.is_readable() {
                let mut buf: [u8; 100000] = [0; 100000];
                match stream.try_read(&mut buf) {
                    Ok(_) => {
                        let msg = Message::from_bytes(&buf)?;
                        if msg.id == Some(id) {
                            return Ok(msg);
                        }
                    }
                    Err(ref e) if e.kind() == ErrorKind::WouldBlock => {
                        continue;
                    }
                    Err(e) => {
                        return Err(ConnectionErr::TokioReadError(e));
                    }
                };
            }
        }
    }

    pub async fn send_bitfield(&mut self, bitfield: &Bytes) -> Result<Message, ConnectionErr> {
        let msg = Message {
            length: (bitfield.len() + 1) as u32,
            id: Some(5),
            payload: Some(bitfield.clone()),
        };

        self.send_message(&msg).await
    }

    /// Establishes a connection and performs handshake with peer
    pub async fn connect(self: &mut Self, handshake: &Handshake) -> Result<(), ConnectionErr> {
        let mut stream = TcpStream::connect(format!("{}:{}", self.ip, self.port))
            .await
            .map_err(|e| ConnectionErr::TokioConnectError(e))?;

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
                        return Err(ConnectionErr::TokioWriteError(e));
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
                        return Err(ConnectionErr::TokioReadError(e));
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
                        return Err(ConnectionErr::TokioWriteError(e));
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
                        return Err(ConnectionErr::TokioReadError(e));
                    }
                };
            }
        }
    }
}
