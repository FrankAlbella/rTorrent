# rTORRENT
A BitTorrent application made using Rust.

## Architecture
rTorrent is divided into 3 main parts: librtorrent, rtorrentd, and rtorrent-cli. 

### librtorrent 
Contains the handles the implementation details for the torrenting process. This includes:
* Connecting to trackers
* Connecting to peers
* Communication between daemon and user interfacts

### libtorrentd
A headless daemon that runs in the background. Handles and manages peer connections and downloads.

### libtorrent-cli
A client application that communicates via librtorrent to libtorrentd. Used to:
* Display progress and status of downloads
* Add or remove current torrents
