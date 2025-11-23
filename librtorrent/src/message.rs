use bytes::{BufMut, Bytes, BytesMut};
use thiserror::Error;

const LENGTH_SIZE: usize = 4;
const ID_SIZE: usize = 1;
const HEADER_SIZE: usize = LENGTH_SIZE + ID_SIZE;

#[derive(Debug, Clone)]
pub struct Message {
    pub length: u32,
    pub id: Option<u8>,
    pub payload: Option<Bytes>,
}

pub enum MessageType {
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

//TODO: improve memory usage
impl Message {
    // TODO: automatically calculate length
    pub fn new(length: u32, id: Option<u8>, payload: Option<Bytes>) -> Self {
        Message {
            length,
            id,
            payload,
        }
    }

    pub fn to_bytes(&self) -> Bytes {
        let mut buf = BytesMut::with_capacity(LENGTH_SIZE + self.length as usize);

        buf.put_u32(self.length);

        if let Some(id) = self.id {
            buf.put_u8(id);
        }

        if let Some(bytes) = &self.payload {
            buf.extend_from_slice(&bytes);
        }

        buf.freeze()
    }

    pub fn from_bytes(bytes: &[u8]) -> Result<Message, MessageErr> {
        if bytes.len() < LENGTH_SIZE {
            return Err(MessageErr::InvalidMessageLength);
        }

        let length = u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]);

        if length == 0 as u32 {
            return Ok(Message {
                length,
                id: None,
                payload: None,
            });
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

        Ok(Message {
            length,
            id: Some(id),
            payload,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn message_deserialize_success() {
        let bytes = [0, 0, 0, 5, 5, 1, 1, 1, 1];
        let message = Message::from_bytes(&bytes).unwrap();
        assert_eq!(message.length, 5);
        assert_eq!(message.id, Some(5));
        assert_eq!(message.payload, Some(Bytes::from_static(&[1, 1, 1, 1])));
    }

    #[test]
    fn message_serialize_success() {
        let message = Message {
            length: 5,
            id: Some(5),
            payload: Some(Bytes::from_static(&[1, 1, 1, 1])),
        };

        let serialized = message.to_bytes();
        let expected = Bytes::copy_from_slice(&[0, 0, 0, 5, 5, 1, 1, 1, 1]);
        assert_eq!(serialized, expected);
    }
}
