use crate::{
    bencode::{BencodeMap, BencodeMapDecoder, BencodeParseErr},
    meta_info::FromBencodeTypeErr,
    peer::Peer,
};
use reqwest::{Client, Url};
use serde::Serialize;
use std::str::FromStr;
use thiserror::Error;
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
pub enum GetResponse {
    Success { interval: i64, peers: Vec<Peer> },
    Failure(String),
}

#[derive(Serialize, Clone, Debug)]
pub enum TrackerEvent {
    #[serde(rename = "started")]
    Started,
    #[serde(rename = "completed")]
    Completed,
    #[serde(rename = "stopped")]
    Stopped,
}

#[derive(Debug, Error)]
pub enum TrackerErr {
    #[error("Invalid meta info")]
    InvalidMetaInfo,
    #[error("URL parse error {0}")]
    UrlParseError(#[from] ParseError),
    #[error("Bencode parse error {0}")]
    BencodeParseErr(#[from] BencodeParseErr),
    #[error("FromBencodeTypeErr {0}")]
    FromBencodeTypeErr(#[from] FromBencodeTypeErr),
    #[error("Reqwest error {0}")]
    ReqwestError(reqwest::Error),
    #[error("Serde error {0}")]
    SerdeErr(serde_qs::Error),
    #[error("Tracker error {0}")]
    TrackerError(String),
}

impl TryFrom<&BencodeMap> for GetResponse {
    type Error = FromBencodeTypeErr;

    fn try_from(bencode_map: &BencodeMap) -> Result<Self, Self::Error> {
        let failure_reason: Option<String> = bencode_map.get_decode(FAILURE_REASON_KEY);

        if let Some(failure_reason) = failure_reason {
            return Ok(Self::Failure(failure_reason));
        }

        let interval: i64 =
            bencode_map
                .get_decode(INTERVAL_KEY)
                .ok_or(FromBencodeTypeErr::MissingValue(
                    "Missing interval value from tracker response".to_string(),
                ))?;

        let peers = bencode_map
            .get_decode::<Vec<BencodeMap>>(PEERS_KEY)
            .map(|peer_maps| Peer::from_bencodemap_list(&peer_maps))
            .transpose()?
            .ok_or_else(|| {
                FromBencodeTypeErr::MissingValue(
                    "Missing peers value from tracker response".to_string(),
                )
            })?;

        Ok(GetResponse::Success { interval, peers })
    }
}

impl TryFrom<&MetaInfo> for GetRequest {
    type Error = TrackerErr;

    fn try_from(meta_info: &MetaInfo) -> Result<Self, Self::Error> {
        let left = match meta_info.info.is_single_or_multi_file() {
            TorrentType::SingleFile => meta_info.info.length.ok_or(TrackerErr::InvalidMetaInfo)?,
            TorrentType::MultiFile => todo!("Add support for multi-file torrents"),
        };

        Ok(GetRequest {
            peer_id: "-RB0001-000000000001".to_string(),
            ip: None,
            port: 6881,
            uploaded: 0,
            downloaded: 0,
            left,
            event: None,
        })
    }
}

pub async fn send_get_request(
    meta_info: &MetaInfo,
    event: TrackerEvent,
) -> Result<GetResponse, TrackerErr> {
    let url = construct_get_url(meta_info, &event)?;
    let client = Client::new();
    let res = client
        .get(url)
        .send()
        .await
        .map_err(TrackerErr::ReqwestError)?
        .bytes()
        .await
        .map_err(TrackerErr::ReqwestError)?;

    let map = BencodeMap::try_decode(&res)?;

    Ok(GetResponse::try_from(&map)?)
}

fn construct_get_url(meta_info: &MetaInfo, event: &TrackerEvent) -> Result<Url, TrackerErr> {
    let mut payload = GetRequest::try_from(meta_info)?;
    payload.event = Some(event.clone());
    let params = serde_qs::to_string(&payload).map_err(TrackerErr::SerdeErr)?;
    let announce = match meta_info.announce.clone() {
        Some(url) => url,
        _ => todo!("Support for torrents without announce field"),
    };

    Url::from_str(&format!(
        "{}?{}&info_hash={}",
        announce,
        params,
        byte_serialize(&meta_info.hash).collect::<String>(),
    ))
    .map_err(TrackerErr::UrlParseError)
}
