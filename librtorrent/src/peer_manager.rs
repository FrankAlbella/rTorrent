use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{mpsc, Mutex};

use crate::{
    meta_info::MetaInfo,
    peer::{Peer, PeerEvent},
    piece_manager::PieceManager,
    tracker::{self, TrackerErr},
};

const DEFAULT_INTERVAL: usize = 600;

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
    new_peer_interval: usize,
    piece_manager: Arc<PieceManager>,
}

impl PeerManager {
    pub async fn new(meta_info: Arc<MetaInfo>) -> Self {
        let (tx, rx) = mpsc::channel::<PeerEvent>(64);
        PeerManager {
            peers: Arc::new(Mutex::new(Vec::new())),
            sender: tx,
            receiver: rx,
            meta_info: meta_info.clone(),
            new_peer_interval: DEFAULT_INTERVAL,
            piece_manager: Arc::new(PieceManager::new(&meta_info.clone()).await),
        }
    }

    pub async fn start(&mut self) -> Result<(), PeerManagerError> {
        //todo!("Add peer manager start function")
        let peers = self.get_new_peers().await?;
        let hash = Arc::new(self.meta_info.hash);

        let mut handles = Vec::new();
        for mut peer in peers {
            let pm = self.piece_manager.clone();
            let h = hash.clone();
            handles.push(tokio::spawn(async move {
                match peer.start(&pm, h).await {
                    Ok(_) => {}
                    Err(err) => {
                        println!("Error starting peer: {}", err);
                    }
                }
            }));
        }

        for handle in handles {
            handle.await.expect("Task panicked");
        }

        // tokio::spawn(async {
        //     self.main_loop().await;
        // });
        //
        Ok(())
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

    /// Sends a peer request to the tracker and returns a vector of Peers
    /// Also updates self.new_peer_inverval (in seconds) from tracker response
    async fn get_new_peers(&mut self) -> Result<Vec<Peer>, PeerManagerError> {
        let response =
            tracker::send_get_request(&self.meta_info, tracker::TrackerEvent::Started).await;
        match response {
            Ok(res) => {
                if let Some(interval) = res.interval {
                    self.new_peer_interval = interval as usize;
                }

                if let Some(peers_vec) = res.peers {
                    Ok(peers_vec)
                } else {
                    Err(PeerManagerError::ConnectionFailed)
                }
            }
            Err(err) => Err(PeerManagerError::TrackerError(err)),
        }
    }
}
