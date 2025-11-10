use tokio::io::AsyncWriteExt;
use tokio::io::ErrorKind;
use tokio::io::Interest;
use tokio::net::TcpStream;

use crate::bencode::BencodeMap;
use crate::bencode::BencodeMapDecoder;
use crate::handshake::Handshake;
use crate::meta_info::FromBencodeTypeErr;
use crate::meta_info::FromBencodemap;

// Peer keys
const PEER_ID_KEY: &str = "peer id";
const IP_KEY: &str = "ip";
const PORT_KEY: &str = "port";

#[derive(Debug, Clone)]
pub struct Peer {
    pub peer_id: Option<String>,
    pub ip: String,
    pub port: i64,
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

        Ok(Peer {
            peer_id: peer_id,
            ip: ip,
            port: port,
        })
    }

    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool {
        bencode_map.contains_key(IP_KEY.as_bytes()) && bencode_map.contains_key(PORT_KEY.as_bytes())
    }
}

#[derive(Debug)]
pub enum ConnectionErr {
    TokioError(std::io::Error),
    InvalidHandshake,
}

impl Peer {
    pub fn from_bencodemap_list(
        bencode_map: &Vec<BencodeMap>,
    ) -> Result<Vec<Self>, FromBencodeTypeErr> {
        bencode_map
            .iter()
            .map(|x| Peer::from_bencodemap(x))
            .collect()
    }

    /// Establishes a connection and performs handshake with peer
    pub async fn connect(self: &Self, handshake: &Handshake) -> Result<TcpStream, ConnectionErr> {
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
                        Ok(their_hs) if their_hs.is_valid(&handshake) => return Ok(stream),
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
}
