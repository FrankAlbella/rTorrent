use bytes::{Buf, BufMut, Bytes, BytesMut};
use thiserror::Error;
use tokio::{io::AsyncReadExt, net::TcpStream};

const LENGTH_SIZE: usize = 4;
const ID_SIZE: u32 = 1;
const HEADER_SIZE: usize = LENGTH_SIZE + ID_SIZE as usize;

#[derive(Debug, Clone, PartialEq)]
pub enum Message {
    KeepAlive,
    Choke,
    Unchoke,
    Interested,
    NotInterested,
    Have {
        index: u32,
    },
    Bitfield {
        bitfield: Bytes,
    },
    Request {
        index: u32,
        begin: u32,
        length: u32,
    },
    Piece {
        index: u32,
        begin: u32,
        block: Bytes,
    },
    Cancel {
        index: u32,
        begin: u32,
        length: u32,
    },
    Port {
        port: u16,
    },
}

pub enum MessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

#[derive(Debug, Error)]
pub enum MessageErr {
    #[error("Invalid message length")]
    InvalidMessageLength,
    #[error("Invalid message Id")]
    InvalidMessageId,
    #[error("IO error {0}")]
    IoError(#[from] std::io::Error),
    #[error("Missing payload")]
    MissingPayload,
    #[error("Invalid payload")]
    InvalidPayload,
    #[error("Invalid message")]
    InvalidMessage,
}

impl TryFrom<u8> for MessageId {
    type Error = MessageErr;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(MessageId::Choke),
            1 => Ok(MessageId::Unchoke),
            2 => Ok(MessageId::Interested),
            3 => Ok(MessageId::NotInterested),
            4 => Ok(MessageId::Have),
            5 => Ok(MessageId::Bitfield),
            6 => Ok(MessageId::Request),
            7 => Ok(MessageId::Piece),
            8 => Ok(MessageId::Cancel),
            9 => Ok(MessageId::Port),
            _ => Err(MessageErr::InvalidMessageId),
        }
    }
}

//TODO: improve memory usage (?)
impl Message {
    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::new();

        match self {
            Message::KeepAlive => {
                buf.put_u32(0);
                buf.freeze()
            }
            Message::Choke => {
                buf.put_u32(ID_SIZE);
                buf.put_u8(MessageId::Choke as u8);
                buf.freeze()
            }
            Message::Unchoke => {
                buf.put_u32(ID_SIZE);
                buf.put_u8(MessageId::Unchoke as u8);
                buf.freeze()
            }
            Message::Interested => {
                buf.put_u32(ID_SIZE);
                buf.put_u8(MessageId::Interested as u8);
                buf.freeze()
            }
            Message::NotInterested => {
                buf.put_u32(ID_SIZE);
                buf.put_u8(MessageId::NotInterested as u8);
                buf.freeze()
            }
            Message::Have { index } => {
                buf.put_u32(5);
                buf.put_u8(MessageId::Have as u8);
                buf.put_u32(*index);
                buf.freeze()
            }
            Message::Bitfield { bitfield } => {
                buf.put_u32(ID_SIZE + bitfield.len() as u32);
                buf.put_u8(MessageId::Bitfield as u8);
                buf.extend_from_slice(&bitfield);
                buf.freeze()
            }
            Message::Request {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u8(MessageId::Request as u8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
                buf.freeze()
            }
            Message::Piece {
                index,
                begin,
                block,
            } => {
                buf.put_u32(9 + block.len() as u32);
                buf.put_u8(MessageId::Piece as u8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.extend_from_slice(&block);
                buf.freeze()
            }
            Message::Cancel {
                index,
                begin,
                length,
            } => {
                buf.put_u32(13);
                buf.put_u8(MessageId::Cancel as u8);
                buf.put_u32(*index);
                buf.put_u32(*begin);
                buf.put_u32(*length);
                buf.freeze()
            }
            Message::Port { port } => {
                buf.put_u32(3);
                buf.put_u8(MessageId::Port as u8);
                buf.put_u16(*port);
                buf.freeze()
            }
        }
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Message, MessageErr> {
        if bytes.len() < LENGTH_SIZE {
            return Err(MessageErr::InvalidMessageLength);
        }

        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        if length == 0 as u32 {
            return Ok(Message::KeepAlive);
        }

        let id = bytes[4];

        if id > 9 {
            return Err(MessageErr::InvalidMessageId);
        }

        if bytes.len() < LENGTH_SIZE + length as usize {
            return Err(MessageErr::InvalidMessageLength);
        }

        let final_index = LENGTH_SIZE + length as usize;

        let payload = if length > 1 {
            Some(Bytes::copy_from_slice(&bytes[HEADER_SIZE..final_index]))
        } else {
            None
        };

        let msg_id = MessageId::try_from(id)?;
        Ok(match msg_id {
            MessageId::Choke => Message::Choke,
            MessageId::Unchoke => Message::Unchoke,
            MessageId::Interested => Message::Interested,
            MessageId::NotInterested => Message::NotInterested,
            MessageId::Have => {
                let mut buf = payload.ok_or(MessageErr::MissingPayload)?;
                Message::Have {
                    index: buf.get_u32(),
                }
            }
            MessageId::Bitfield => {
                let buf = payload.ok_or(MessageErr::MissingPayload)?;
                Message::Bitfield { bitfield: buf }
            }
            MessageId::Request => {
                let mut buf = payload.ok_or(MessageErr::MissingPayload)?;
                Message::Request {
                    index: buf.get_u32(),
                    begin: buf.get_u32(),
                    length: buf.get_u32(),
                }
            }
            MessageId::Piece => {
                let mut buf = payload.ok_or(MessageErr::MissingPayload)?;
                Message::Piece {
                    index: buf.get_u32(),
                    begin: buf.get_u32(),
                    block: buf,
                }
            }
            MessageId::Cancel => {
                let mut buf = payload.ok_or(MessageErr::MissingPayload)?;
                Message::Cancel {
                    index: buf.get_u32(),
                    begin: buf.get_u32(),
                    length: buf.get_u32(),
                }
            }
            MessageId::Port => {
                let mut buf = payload.ok_or(MessageErr::MissingPayload)?;
                Message::Port {
                    port: buf.get_u16(),
                }
            }
        })
    }

    pub async fn from_stream(stream: &mut TcpStream) -> Result<Message, MessageErr> {
        let mut len_buf = [0u8; LENGTH_SIZE];
        stream.read_exact(&mut len_buf).await?;

        let length = u32::from_be_bytes(len_buf);

        if length == 0 {
            return Ok(Message::KeepAlive);
        }

        let mut payload_buf = vec![0u8; length as usize];
        stream.read_exact(&mut payload_buf).await?;

        let mut full_buf = BytesMut::with_capacity(LENGTH_SIZE + length as usize);
        full_buf.extend_from_slice(&len_buf);
        full_buf.extend_from_slice(&payload_buf);

        Self::from_bytes(&full_buf.freeze())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_deserialize_success() {
        let bytes = [0, 0, 0, 5, 5, 1, 1, 1, 1];
        let message = Message::from_bytes(&bytes).unwrap();
        assert_eq!(
            message,
            Message::Bitfield {
                bitfield: Bytes::from_static(&[1, 1, 1, 1])
            }
        );
    }

    #[test]
    fn message_serialize_success() {
        let message = Message::Bitfield {
            bitfield: Bytes::from_static(&[1, 1, 1, 1]),
        };

        let serialized = message.to_bytes();
        let expected = Bytes::copy_from_slice(&[0, 0, 0, 5, 5, 1, 1, 1, 1]);
        assert_eq!(serialized, expected);
    }
}
