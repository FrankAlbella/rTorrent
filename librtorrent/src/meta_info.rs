use crate::bencode::{BencodeGetErr, BencodeMap, BencodeMapDecoder, BencodeMapEncoder};
use sha1::{Digest, Sha1};
use std::path::PathBuf;
use thiserror::Error;

// Keys for the root of the meta info file
const ANNOUNCE_KEY: &str = "announce";
const INFO_KEY: &str = "info";
const NODES_KEY: &str = "nodes";
const ANNOUNCE_LIST_KEY: &str = "announce-list";
const URL_LIST_KEY: &str = "url-list";

// Keys for the info dict in the file
const NAME_KEY: &str = "name";
const PIECE_LENGTH_KEY: &str = "piece length";
const PIECES_KEY: &str = "pieces";
const LENGTH_KEY: &str = "length";
const FILES_KEY: &str = "files";
const PRIVATE_KEY: &str = "private";

// Heys for the files dict
const PATH_KEY: &str = "path";

const HASH_SIZE: usize = 20;

#[derive(Debug, Error)]
pub enum FromBencodeTypeErr {
    #[error("Map is missing required values {0}")]
    MissingValue(String),
    #[error("Invalid value for {0}")]
    InvalidValue(String),
    #[error("Failed to get bencode")]
    BencodeGetErr(#[from] BencodeGetErr),
}

#[derive(Debug, Clone)]
pub struct MetaInfo {
    pub announce: Option<String>,
    pub info: TorrentInfo,
    //BEP-0005
    pub nodes: Option<Vec<String>>,
    //BEP-0012
    pub announce_list: Option<Vec<String>>,
    //BEP-0019
    pub url_list: Option<Vec<String>>,
    pub hash: [u8; 20],
}

#[derive(Debug, Clone)]
pub struct TorrentInfo {
    pub name: String,
    pub piece_length: i64,
    pub pieces: Vec<u8>,
    pub file_layout: FileLayout,
    //BEP-0027
    pub private: Option<i64>,
}

#[derive(Debug, Clone)]
pub enum FileLayout {
    SingleFile { length: i64 },
    MultiFile { files: Vec<FileInfo> },
}

#[derive(Debug, Clone)]
pub struct FileInfo {
    pub length: i64,
    pub path: Vec<PathBuf>,
}

impl TorrentInfo {
    pub fn get_piece_hash(&self, piece_index: usize) -> Option<[u8; HASH_SIZE]> {
        let start = piece_index * HASH_SIZE;
        let end = start + HASH_SIZE;

        self.pieces.get(start..end)?.try_into().ok()
    }
}

impl TryFrom<&BencodeMap> for FileInfo {
    type Error = FromBencodeTypeErr;
    fn try_from(bencode_map: &BencodeMap) -> Result<Self, Self::Error> {
        let length: i64 = bencode_map
            .get_decode(LENGTH_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(LENGTH_KEY)))?;
        let path: Vec<PathBuf> = bencode_map
            .get_decode(PATH_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(PATH_KEY)))?;

        Ok(FileInfo { length, path })
    }
}

impl TryFrom<&BencodeMap> for TorrentInfo {
    type Error = FromBencodeTypeErr;
    fn try_from(bencode_map: &BencodeMap) -> Result<Self, Self::Error> {
        let name: String = bencode_map
            .get_decode(NAME_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(NAME_KEY)))?;
        let piece_length: i64 =
            bencode_map
                .get_decode(PIECE_LENGTH_KEY)
                .ok_or(FromBencodeTypeErr::MissingValue(String::from(
                    PIECE_LENGTH_KEY,
                )))?;
        let pieces: Vec<u8> = bencode_map
            .get_decode(PIECES_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(PIECES_KEY)))?;
        let private: Option<i64> = bencode_map.get_decode(PRIVATE_KEY);

        let length: Option<i64> = bencode_map.get_decode(LENGTH_KEY);
        let files: Option<Vec<BencodeMap>> = bencode_map.get_decode(FILES_KEY);
        let file_layout = match (length, files) {
            (Some(len), None) => FileLayout::SingleFile { length: len },
            (None, Some(files)) => FileLayout::MultiFile {
                files: files
                    .iter()
                    .map(FileInfo::try_from)
                    .collect::<Result<Vec<_>, _>>()?,
            },
            _ => {
                return Err(FromBencodeTypeErr::InvalidValue(
                    "length and files".to_string(),
                ))
            }
        };

        Ok(TorrentInfo {
            name,
            piece_length,
            pieces,
            file_layout,
            private,
        })
    }
}

impl TryFrom<&BencodeMap> for MetaInfo {
    type Error = FromBencodeTypeErr;
    fn try_from(bencode_map: &BencodeMap) -> Result<Self, Self::Error> {
        let announce: Option<String> = bencode_map.get_decode(ANNOUNCE_KEY);
        let nodes: Option<Vec<String>> = bencode_map.get_decode(NODES_KEY);
        let announce_list: Option<Vec<String>> = bencode_map.get_decode(ANNOUNCE_LIST_KEY);
        let url_list: Option<Vec<String>> = bencode_map.get_decode(URL_LIST_KEY);

        if announce.is_none() && announce_list.is_none() && nodes.is_none() && url_list.is_none() {
            return Err(FromBencodeTypeErr::MissingValue(
                "announce, nodes, announce_list, or url_list key required for MetaInfo".to_string(),
            ));
        }

        let info: BencodeMap = bencode_map
            .get_decode(INFO_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(INFO_KEY.to_string()))?;

        Ok(MetaInfo {
            announce,
            info: TorrentInfo::try_from(&info)?,
            nodes,
            announce_list,
            url_list,
            hash: Sha1::digest(info.get_encode()).into(),
        })
    }
}

impl TorrentInfo {
    pub fn get_piece_hashes(&self) -> Vec<[u8; 20]> {
        self.pieces
            .chunks(HASH_SIZE)
            .map(|chunk| {
                let mut hash = [0u8; HASH_SIZE];
                hash.copy_from_slice(chunk);
                hash
            })
            .collect()
    }
}
