use librtorrent::session::Session;

#[tokio::main]
async fn main() {
    let mut session = Session::new();
    session.add_torrent("test/torrent_files/debian-13.1.0-amd64-netinst.iso.torrent");
    session.start().await;
}
