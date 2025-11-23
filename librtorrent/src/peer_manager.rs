use std::sync::Arc;

use bytes::{Bytes, BytesMut};
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};

use crate::{
    handshake::Handshake,
    meta_info::MetaInfo,
    peer::{Peer, PeerEvent},
    tracker::{self, TrackerErr},
};

#[derive(Debug, Error)]
pub enum PeerManagerError {
    #[error("Failed to connect to peers")]
    ConnectionFailed,
    #[error("Failed to start peer")]
    PeerStartFailed,
    #[error("Tracker error {0}")]
    TrackerError(#[from] TrackerErr),
}

#[derive(Debug)]
pub struct PeerManager {
    peers: Arc<Mutex<Vec<Peer>>>,
    sender: mpsc::Sender<PeerEvent>,
    receiver: mpsc::Receiver<PeerEvent>,
    meta_info: Arc<MetaInfo>,
}

impl PeerManager {
    pub fn new(meta_info: Arc<MetaInfo>) -> Self {
        let (tx, rx) = mpsc::channel::<PeerEvent>(64);
        PeerManager {
            peers: Arc::new(Mutex::new(Vec::new())),
            sender: tx,
            receiver: rx,
            meta_info: meta_info.clone(),
        }
    }

    pub async fn start(&self) {
        todo!("Add peer manager start function")
        // tokio::spawn(async move {
        //     let peers = self.get_new_peers().await;

        //     for peer in peers.unwrap() {
        //         peer.start().await;
        //     }

        //     //self.connect_to_peers(peers).await;
        // });

        // tokio::spawn(async {
        //     self.main_loop().await;
        // });
    }

    async fn main_loop(&mut self) {
        // Main loop for the peer manager
        loop {
            if let Some(event) = self.receiver.recv().await {
                match event {
                    _ => todo!("Add peer manager reciever event handling"),
                }
            }
        }
    }

    async fn get_new_peers(&self) -> Result<Vec<Peer>, PeerManagerError> {
        let response = tracker::send_get_request(&self.meta_info).await;
        match response {
            Ok(res) => {
                if let Some(peers_vec) = res.peers {
                    Ok(peers_vec)
                } else {
                    Err(PeerManagerError::ConnectionFailed)
                }
            }
            Err(err) => Err(PeerManagerError::TrackerError(err)),
        }
    }

    pub async fn connect_to_peers(&self, peers: Vec<Peer>) {
        let mut handles = Vec::new();

        for mut peer in peers {
            let hash_clone = self.meta_info.hash.clone();
            let peers_vec_clone = self.peers.clone();
            let bitfield = self.get_bitfield();
            let piece_length = self.meta_info.info.piece_length;
            let piece_hash = self.meta_info.info.get_piece_hash(0).unwrap();
            handles.push(tokio::spawn(async move {
                match peer.connect(&Handshake::new(hash_clone, [0; 20])).await {
                    Ok(_) => {
                        //let mut peers = peers_vec_clone.lock().await;
                        //peers.push(peer.start());
                        println!("Connected to peer");
                    }
                    Err(e) => {
                        println!("Failed to connect to peer: {:#?}", e);
                        return;
                    }
                }

                match peer.send_bitfield(&bitfield).await {
                    Ok(_) => println!("Bitfield received!"),
                    Err(e) => {
                        println!("Failed to send bitfield: {:#?}", e);
                        return;
                    }
                }

                match peer
                    .download_piece(0, piece_length as u64, piece_hash)
                    .await
                {
                    Ok(result) => println!("Downloaded piece: {:#?}", result),
                    Err(e) => println!("Failed to download piece: {:#?}", e),
                }
            }));
        }

        for handle in handles {
            handle.await.expect("Task panicked");
        }
    }

    //TODO: Update and move to piece manager
    //TODO: Keep track of what pieces we have
    pub fn get_bitfield(&self) -> Bytes {
        let total_length = self.meta_info.info.length.unwrap();
        let piece_length = self.meta_info.info.piece_length;
        let num_pieces = (total_length + piece_length - 1) / piece_length;

        let num_bytes = (num_pieces + 7) / 8;

        let mut buf_bitfield = BytesMut::new();
        buf_bitfield.resize(num_bytes as usize, 0);
        buf_bitfield.freeze()
    }
}
