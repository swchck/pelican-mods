//! Finding a co-op partner's mod list on the local network.
//!
//! A host that shares answers a UDP broadcast probe on [`PORT`] with its name and a TCP
//! port; connecting there returns the active profile as a mod list and closes. Nothing
//! but the list goes out (names, versions, update keys), sharing is off unless the
//! player turns it on, and nothing that arrives is installed without them.

use std::net::{Ipv4Addr, SocketAddr};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{TcpListener, TcpStream, UdpSocket};

use crate::error::{AppError, AppResult};
use crate::modlist::ModList;

/// The UDP port hosts listen for probes on.
pub const PORT: u16 = 24643;
const PROBE: &[u8] = b"pelican-modlist?1";
/// A list of a few hundred mods is well under this; anything bigger isn't one.
const MAX_LIST: u64 = 4 << 20;
const IO_TIMEOUT: Duration = Duration::from_secs(5);

/// A host that answered a probe.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Host {
    /// The host's profile name, and its farmer where it has a save.
    pub name: String,
    #[serde(default)]
    pub addr: String,
    pub port: u16,
}

/// Builds the list to hand out, fresh for each partner so it follows the host's changes.
pub type ListSource = Arc<dyn Fn() -> AppResult<Vec<u8>> + Send + Sync>;

/// A running share; dropping it stops answering.
pub struct Share {
    tasks: Vec<tauri::async_runtime::JoinHandle<()>>,
}

impl Drop for Share {
    fn drop(&mut self) {
        for t in &self.tasks {
            t.abort();
        }
    }
}

impl Share {
    /// Starts answering probes on `udp_port` as `name`, handing out what `list` builds.
    pub async fn start(udp_port: u16, name: String, list: ListSource) -> AppResult<Self> {
        let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, udp_port)).await?;
        let tcp = TcpListener::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
        let announce = serde_json::to_vec(&Host {
            name,
            addr: String::new(),
            port: tcp.local_addr()?.port(),
        })
        .expect("host serialize");
        Ok(Self {
            tasks: vec![
                tauri::async_runtime::spawn(respond(udp, announce)),
                tauri::async_runtime::spawn(serve(tcp, list)),
            ],
        })
    }
}

async fn respond(udp: UdpSocket, announce: Vec<u8>) {
    let mut buf = [0u8; 64];
    while let Ok((n, from)) = udp.recv_from(&mut buf).await {
        if &buf[..n] == PROBE {
            let _ = udp.send_to(&announce, from).await;
        }
    }
}

async fn serve(tcp: TcpListener, list: ListSource) {
    while let Ok((mut stream, _)) = tcp.accept().await {
        let list = list.clone();
        tauri::async_runtime::spawn(async move {
            let Ok(Ok(body)) = tauri::async_runtime::spawn_blocking(move || list()).await else {
                return;
            };
            let _ = tokio::time::timeout(IO_TIMEOUT, async {
                stream.write_all(&body).await?;
                stream.shutdown().await
            })
            .await;
        });
    }
}

/// Asks `target` (the broadcast address, or one machine) who shares, collecting answers
/// for `wait`.
pub async fn discover(target: SocketAddr, wait: Duration) -> AppResult<Vec<Host>> {
    let udp = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).await?;
    udp.set_broadcast(true)?;
    // a denied Local Network permission looks exactly like this to the socket
    udp.send_to(PROBE, target)
        .await
        .map_err(|e| match e.kind() {
            std::io::ErrorKind::HostUnreachable => AppError::LocalNetworkDenied,
            _ => e.into(),
        })?;
    let mut hosts: Vec<Host> = Vec::new();
    let mut buf = [0u8; 2048];
    let deadline = tokio::time::Instant::now() + wait;
    while let Ok(Ok((n, from))) = tokio::time::timeout_at(deadline, udp.recv_from(&mut buf)).await {
        let Ok(mut host) = serde_json::from_slice::<Host>(&buf[..n]) else {
            continue;
        };
        // the address is where the answer came from, never what the answer claims
        host.addr = from.ip().to_string();
        if !hosts
            .iter()
            .any(|h| h.addr == host.addr && h.port == host.port)
        {
            hosts.push(host);
        }
    }
    Ok(hosts)
}

/// Fetches the list a host at `addr` hands out.
pub async fn fetch(addr: SocketAddr) -> AppResult<ModList> {
    let read = async {
        let stream = TcpStream::connect(addr).await?;
        let mut body = Vec::new();
        stream.take(MAX_LIST + 1).read_to_end(&mut body).await?;
        Ok::<_, std::io::Error>(body)
    };
    let body = tokio::time::timeout(IO_TIMEOUT, read)
        .await
        .map_err(|_| AppError::BadModList(format!("{addr}: no answer")))??;
    if body.len() as u64 > MAX_LIST {
        return Err(AppError::BadModList(format!("{addr}: too big")));
    }
    crate::modlist::parse(&body, &addr.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_a_host_and_fetches_its_list() {
        tauri::async_runtime::block_on(async {
            let list = crate::modlist::build("Ферма", &[], &[], &Default::default(), None, None);
            let body = serde_json::to_vec(&list).unwrap();
            // a free port for the probe, so the test never fights a running app for PORT
            let port = std::net::UdpSocket::bind("127.0.0.1:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            let share = Share::start(port, "Аня".into(), Arc::new(move || Ok(body.clone())))
                .await
                .unwrap();

            let hosts = discover(([127, 0, 0, 1], port).into(), Duration::from_millis(500))
                .await
                .unwrap();
            assert_eq!(hosts.len(), 1);
            assert_eq!(
                (hosts[0].name.as_str(), hosts[0].addr.as_str()),
                ("Аня", "127.0.0.1")
            );
            let got = fetch(([127, 0, 0, 1], hosts[0].port).into()).await.unwrap();
            assert_eq!(got.name, "Ферма");

            drop(share);
            tokio::time::sleep(Duration::from_millis(50)).await;
            let gone = discover(([127, 0, 0, 1], port).into(), Duration::from_millis(200))
                .await
                .unwrap();
            assert!(gone.is_empty(), "a stopped share stays quiet");
        });
    }

    // needs a network that takes broadcasts and a Local Network grant for the test binary,
    // which macOS gives only to a bundled app; it fails with LocalNetworkDenied otherwise:
    // cargo test -- --ignored broadcast_reaches_this_machine
    #[test]
    #[ignore]
    fn broadcast_reaches_this_machine() {
        tauri::async_runtime::block_on(async {
            let port = std::net::UdpSocket::bind("0.0.0.0:0")
                .unwrap()
                .local_addr()
                .unwrap()
                .port();
            let _share = Share::start(port, "me".into(), Arc::new(|| Ok(Vec::new())))
                .await
                .unwrap();
            let hosts = discover(
                (Ipv4Addr::BROADCAST, port).into(),
                Duration::from_millis(800),
            )
            .await
            .unwrap();
            eprintln!("{hosts:?}");
            assert!(!hosts.is_empty());
        });
    }
}
