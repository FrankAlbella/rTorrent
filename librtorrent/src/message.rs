use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

#[derive(Debug)]
pub struct Message {
    pub length: u32,
    pub id: u8,
    pub payload: Option<Bytes>,
}

enum MessageType {
    KeepAlive = -1,
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
}

impl Message {
    pub fn to_bytes(&self) -> Bytes {
        let payload_len;
        if let Some(bytes) = &self.payload {
            payload_len = bytes.len();
        } else {
            payload_len = 0;
        }

        let mut buf = BytesMut::with_capacity(4 + 1 + payload_len);

        buf.put_u32(self.length);
        buf.put_u8(self.id);

        if let Some(bytes) = &self.payload {
            buf.extend_from_slice(&bytes);
        }

        buf.freeze()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Message, MessageErr> {
        if bytes.len() < 4 {
            return Err(MessageErr::InvalidMessageLength);
        }

        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);
        let id = bytes[4];

        if id > 9 {
            return Err(MessageErr::InvalidMessageId);
        }

        if length > bytes.len() as u32 - 5 {
            return Err(MessageErr::InvalidMessageLength);
        }

        let final_index = 5 + length as usize;

        let payload = if length > 1 {
            Some(Bytes::copy_from_slice(&bytes[5..final_index]))
        } else {
            None
        };

        Ok(Message {
            length,
            id,
            payload,
        })
    }
}
