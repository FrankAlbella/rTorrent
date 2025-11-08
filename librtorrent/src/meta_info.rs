use std::path::PathBuf;

use crate::bencode::{BencodeGetErr, BencodeMap, BencodeMapDecoder};

// Keys for the root of the meta info file
const ANNOUNCE_KEY: &str = "announce";
const INFO_KEY: &str = "info";
const NODES_KEY: &str = "nodes";
const ANNOUNCE_LIST_KEY: &str = "announce-list";
const URL_LIST_KEY: &str = "url-list";
const ANNOUNCE_VALUES: [&str; 4] = [ANNOUNCE_KEY, NODES_KEY, ANNOUNCE_LIST_KEY, URL_LIST_KEY];

// Keys for the info dict in the file
const NAME_KEY: &str = "name";
const PIECE_LENGTH_KEY: &str = "piece length";
const PIECES_KEY: &str = "pieces";
const LENGTH_KEY: &str = "length";
const FILES_KEY: &str = "files";
const PRIVATE_KEY: &str = "private";

const INFO_XOR_VALUES: [&str; 2] = [LENGTH_KEY, FILES_KEY];

// Heys for the files dict
const PATH_KEY: &str = "path";

const ERROR_MISSING_VALUE: &str = "Map is missing required values";

#[derive(Debug)]
pub enum FromBencodeTypeErr {
    MissingValue(String),
    BencodeGetErr(BencodeGetErr),
}

impl From<BencodeGetErr> for FromBencodeTypeErr {
    fn from(value: BencodeGetErr) -> Self {
        Self::BencodeGetErr(value)
    }
}

#[derive(Debug)]
pub struct MetaInfo {
    announce: Option<String>,
    info: TorrentInfo,
    //BEP-0005
    nodes: Option<Vec<String>>,
    //BEP-0012
    announce_list: Option<Vec<String>>,
    //BEP-0019
    url_list: Option<Vec<String>>,
}

#[derive(Debug)]
pub struct TorrentInfo {
    name: String,
    piece_length: i64,
    pieces: Vec<u8>,
    length: Option<i64>,
    files: Option<Vec<FileInfo>>,
    //BEP-0027
    private: Option<i64>,
}

#[derive(Debug)]
pub struct FileInfo {
    length: i64,
    path: Vec<PathBuf>,
}

#[derive(Debug)]
pub enum TorrentType {
    SingleFile,
    MultiFile,
}

impl TorrentInfo {
    pub fn is_single_or_multi_file(self: &Self) -> TorrentType {
        match self.files.is_none() {
            true => TorrentType::SingleFile,
            false => TorrentType::MultiFile,
        }
    }
}

pub trait FromBencodemap: Sized {
    fn from_bencodemap(bencode_map: &BencodeMap) -> Result<Self, FromBencodeTypeErr>;
    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool;
}

impl FromBencodemap for FileInfo {
    fn from_bencodemap(bencode_map: &BencodeMap) -> Result<Self, FromBencodeTypeErr> {
        if !Self::is_valid_bencodemap(bencode_map) {
            return Err(FromBencodeTypeErr::MissingValue(String::from(
                ERROR_MISSING_VALUE,
            )));
        }

        let length: i64 = bencode_map
            .get_decode(NAME_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(PIECES_KEY)))?;
        let path: Vec<PathBuf> = bencode_map
            .get_decode(PIECE_LENGTH_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(PIECES_KEY)))?;

        Ok(FileInfo {
            length: length,
            path: path,
        })
    }

    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool {
        bencode_map.contains_key(LENGTH_KEY.as_bytes())
            && bencode_map.contains_key(PATH_KEY.as_bytes())
    }
}

impl FromBencodemap for TorrentInfo {
    fn from_bencodemap(bencode_map: &BencodeMap) -> Result<TorrentInfo, FromBencodeTypeErr> {
        if !Self::is_valid_bencodemap(bencode_map) {
            return Err(FromBencodeTypeErr::MissingValue(String::from(
                ERROR_MISSING_VALUE,
            )));
        }

        bencode_map.print_keys();

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
        let length: Option<i64> = bencode_map.get_decode(LENGTH_KEY);
        let files: Option<Vec<BencodeMap>> = bencode_map.get_decode(FILES_KEY);
        let private: Option<i64> = bencode_map.get_decode(PRIVATE_KEY);

        let mut final_vec = Vec::new();
        if let Some(files_vec) = files {
            while let Some(x) = files_vec.iter().next() {
                final_vec.push(FileInfo::from_bencodemap(x)?);
            }
        }

        let final_file;

        match final_vec.is_empty() {
            true => final_file = None,
            false => final_file = Some(final_vec),
        }

        Ok(TorrentInfo {
            name: name,
            piece_length: piece_length,
            pieces: pieces,
            length: length,
            files: final_file,
            private: private,
        })
    }

    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool {
        let has_values = bencode_map.keys().any(|k| {
            if let Ok(s) = std::str::from_utf8(k) {
                INFO_XOR_VALUES.contains(&s)
            } else {
                false
            }
        });

        if !has_values {
            return false;
        }

        if !bencode_map.contains_key(NAME_KEY.as_bytes()) {
            return false;
        }

        true
    }
}

impl FromBencodemap for MetaInfo {
    fn from_bencodemap(bencode_map: &BencodeMap) -> Result<MetaInfo, FromBencodeTypeErr> {
        if !Self::is_valid_bencodemap(bencode_map) {
            return Err(FromBencodeTypeErr::MissingValue(String::from(
                ERROR_MISSING_VALUE,
            )));
        }

        let announce: Option<String> = bencode_map.get_decode(ANNOUNCE_LIST_KEY);
        let nodes: Option<Vec<String>> = bencode_map.get_decode(NODES_KEY);
        let announce_list: Option<Vec<String>> = bencode_map.get_decode(ANNOUNCE_KEY);
        let url_list: Option<Vec<String>> = bencode_map.get_decode(URL_LIST_KEY);

        let info: BencodeMap = bencode_map
            .get_decode(INFO_KEY)
            .ok_or(FromBencodeTypeErr::MissingValue(String::from(INFO_KEY)))?;

        Ok(MetaInfo {
            announce: announce,
            info: TorrentInfo::from_bencodemap(&info)?,
            nodes: nodes,
            announce_list: announce_list,
            url_list: url_list,
        })
    }

    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool {
        let has_announce = bencode_map.keys().any(|k| {
            if let Ok(s) = std::str::from_utf8(k) {
                ANNOUNCE_VALUES.contains(&s)
            } else {
                false
            }
        });

        if !has_announce {
            return false;
        }

        if !bencode_map.contains_key(INFO_KEY.as_bytes()) {
            return false;
        }

        true
    }
}
