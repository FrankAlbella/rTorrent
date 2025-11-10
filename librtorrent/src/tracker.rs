use crate::{
    bencode::{BencodeMap, BencodeMapDecoder, BencodeParseErr},
    meta_info::{FromBencodeTypeErr, FromBencodemap},
    peer::Peer,
};
use reqwest::{Client, Url};
use serde::Serialize;
use std::str::FromStr;
use url::form_urlencoded::byte_serialize;
use url::ParseError;

use crate::meta_info::{MetaInfo, TorrentType};

// GetResponse keys
const INTERVAL_KEY: &str = "interval";
const PEERS_KEY: &str = "peers";
const FAILURE_REASON_KEY: &str = "failure reason";

// ERRORS

#[derive(Serialize)]
struct GetRequest {
    peer_id: String,
    ip: Option<String>,
    port: u16,
    uploaded: i64,
    downloaded: i64,
    left: i64,
    event: Option<TrackerEvent>,
}

#[derive(Debug)]
pub struct GetResponse {
    pub interval: Option<i64>,
    pub peers: Option<Vec<Peer>>,
    pub failure_reason: Option<String>,
}

#[derive(Serialize)]
enum TrackerEvent {
    Started,
    Completed,
    Stopped,
}

// TODO implement with thiserror::Error
#[derive(Debug)]
pub enum TrackerErr {
    InvalidMetaInfo,
    UrlParseError(ParseError),
    BencodeParseErr(BencodeParseErr),
    FromBencodeTypeErr(FromBencodeTypeErr),
    ReqwestError(reqwest::Error),
    SerdeErr(serde_qs::Error),
}

impl FromBencodemap for GetResponse {
    fn from_bencodemap(bencode_map: &BencodeMap) -> Result<Self, FromBencodeTypeErr> {
        if !Self::is_valid_bencodemap(bencode_map) {
            return Err(FromBencodeTypeErr::MissingValue(String::from(
                "Missing values for GetResponse",
            )));
        }

        let interval: Option<i64> = bencode_map.get_decode(INTERVAL_KEY);
        let failure_reason: Option<String> = bencode_map.get_decode(FAILURE_REASON_KEY);

        let peers: Option<Vec<BencodeMap>> = bencode_map.get_decode(PEERS_KEY);

        let peers_final: Option<Vec<Peer>>;

        match peers {
            Some(x) => peers_final = Some(Peer::from_bencodemap_list(&x)?),
            None => peers_final = None,
        }

        Ok(GetResponse {
            interval: interval,
            peers: peers_final,
            failure_reason: failure_reason,
        })
    }

    fn is_valid_bencodemap(bencode_map: &BencodeMap) -> bool {
        (bencode_map.contains_key(INTERVAL_KEY.as_bytes())
            && bencode_map.contains_key(PEERS_KEY.as_bytes()))
            || bencode_map.contains_key(FAILURE_REASON_KEY.as_bytes())
    }
}

impl GetRequest {
    pub fn from_metainfo(meta_info: &MetaInfo) -> Result<Self, TrackerErr> {
        let left;

        match meta_info.info.is_single_or_multi_file() {
            TorrentType::SingleFile => {
                left = meta_info.info.length.ok_or(TrackerErr::InvalidMetaInfo)?
            }
            TorrentType::MultiFile => todo!("Add support for multi-file torrents"),
        }

        Ok(GetRequest {
            peer_id: "12345678901234567890".to_string(),
            ip: None,
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left: left,
            event: None,
        })
    }
}

pub async fn send_get_request(meta_info: &MetaInfo) -> Result<GetResponse, TrackerErr> {
    let url = construct_get_url(&meta_info)?;
    let client = Client::new();
    let res = client
        .get(url)
        .send()
        .await
        .map_err(|req_error| TrackerErr::ReqwestError(req_error))?
        .bytes()
        .await
        .map_err(|req_error| TrackerErr::ReqwestError(req_error))?;

    let map = BencodeMap::try_decode(&res.into())
        .map_err(|bencode_err| TrackerErr::BencodeParseErr(bencode_err))?;

    let deserial = GetResponse::from_bencodemap(&map)
        .map_err(|from_error| TrackerErr::FromBencodeTypeErr(from_error))?;

    Ok(deserial)
}

fn construct_get_url(meta_info: &MetaInfo) -> Result<Url, TrackerErr> {
    let payload = GetRequest::from_metainfo(&meta_info)?;
    let params =
        serde_qs::to_string(&payload).map_err(|serde_error| TrackerErr::SerdeErr(serde_error))?;
    let announce;

    match meta_info.announce.clone() {
        Some(url) => announce = url,
        _ => todo!("Support for torrents without announce field"),
    }

    Url::from_str(&format!(
        "{}?{}&info_hash={}",
        announce,
        params,
        byte_serialize(&meta_info.hash).collect::<String>(),
    ))
    .map_err(|parse_error| TrackerErr::UrlParseError(parse_error))
}
