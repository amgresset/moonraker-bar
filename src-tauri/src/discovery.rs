//! Finds Moonraker instances on the local network, two ways at once:
//!
//! - mDNS: Moonraker announces `_moonraker._tcp.local.` when `[zeroconf]` is in moonraker.conf.
//! - Subnet scan: every host of the local /24 on port 7125, confirmed with `/server/info`.
//!   This finds printers whose Moonraker does not announce itself (the default).

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};
use std::time::Duration;

use serde::Serialize;
use serde_json::Value;
use tokio::net::TcpStream;
use tokio::task::JoinSet;

const SERVICE: &str = "_moonraker._tcp.local.";
const MDNS_WINDOW: Duration = Duration::from_millis(2500);
const PORT: u16 = 7125;
const CONNECT_TIMEOUT: Duration = Duration::from_millis(450);
const HTTP_TIMEOUT: Duration = Duration::from_millis(1500);

#[derive(Debug, Clone, Serialize)]
pub struct Discovered {
    /// Display name: the Moonraker instance name or the host name.
    pub name: String,
    /// Moonraker base URL, e.g. `http://voron.local:7125`.
    pub url: String,
    pub hostname: String,
    pub ip: String,
    /// `mdns`, `scan`, or `mdns+scan`.
    pub source: String,
    /// Moonraker answered but wants an API key.
    pub needs_api_key: bool,
}

pub async fn discover() -> Vec<Discovered> {
    let (mdns, scan) = tokio::join!(browse_mdns(), scan_subnet());
    let mut by_ip: HashMap<String, Discovered> = HashMap::new();
    for d in scan {
        by_ip.insert(d.ip.clone(), d);
    }
    for m in mdns {
        match by_ip.get_mut(&m.ip) {
            Some(existing) => {
                // mDNS knows the nicer name and a stable .local host name.
                existing.name = m.name;
                existing.url = m.url;
                existing.hostname = m.hostname;
                existing.source = "mdns+scan".into();
            }
            None => {
                by_ip.insert(m.ip.clone(), m);
            }
        }
    }
    let mut list: Vec<Discovered> = by_ip.into_values().collect();
    list.sort_by(|a, b| a.name.to_lowercase().cmp(&b.name.to_lowercase()));
    list
}

// ---------- mDNS ----------

async fn browse_mdns() -> Vec<Discovered> {
    tokio::task::spawn_blocking(|| {
        let Ok(daemon) = mdns_sd::ServiceDaemon::new() else { return Vec::new() };
        let Ok(rx) = daemon.browse(SERVICE) else { return Vec::new() };
        let deadline = std::time::Instant::now() + MDNS_WINDOW;
        let mut found = Vec::new();
        while let Some(left) = deadline.checked_duration_since(std::time::Instant::now()) {
            match rx.recv_timeout(left) {
                Ok(mdns_sd::ServiceEvent::ServiceResolved(info)) => {
                    let Some(ip) = info.get_addresses().iter().find(|a| a.is_ipv4()).copied() else { continue };
                    let host = info.get_hostname().trim_end_matches('.').to_string();
                    // "Voron @ voron._moonraker._tcp.local." -> "Voron"
                    let full = info.get_fullname();
                    let name = full.split(" @ ").next().unwrap_or(full).to_string();
                    let port = info.get_port();
                    let host_for_url = if host.is_empty() { ip.to_string() } else { host.clone() };
                    found.push(Discovered {
                        name,
                        url: format!("http://{host_for_url}:{port}"),
                        hostname: host.trim_end_matches(".local").to_string(),
                        ip: ip.to_string(),
                        source: "mdns".into(),
                        needs_api_key: false,
                    });
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
        let _ = daemon.shutdown();
        found
    })
    .await
    .unwrap_or_default()
}

// ---------- subnet scan ----------

/// The machine's LAN address, found by asking the OS which interface would
/// route to a private address (no packet is sent).
fn local_ipv4() -> Option<Ipv4Addr> {
    let sock = UdpSocket::bind("0.0.0.0:0").ok()?;
    sock.connect("10.255.255.255:1").ok()?;
    match sock.local_addr().ok()?.ip() {
        IpAddr::V4(ip) if ip.is_private() => Some(ip),
        _ => None,
    }
}

async fn scan_subnet() -> Vec<Discovered> {
    let Some(me) = local_ipv4() else { return Vec::new() };
    let [a, b, c, _] = me.octets();

    // 1. Which hosts have port 7125 open.
    let mut probes = JoinSet::new();
    for d in 1u8..=254 {
        let ip = Ipv4Addr::new(a, b, c, d);
        if ip == me {
            continue;
        }
        probes.spawn(async move {
            let addr = SocketAddr::new(IpAddr::V4(ip), PORT);
            matches!(tokio::time::timeout(CONNECT_TIMEOUT, TcpStream::connect(addr)).await, Ok(Ok(_))).then_some(ip)
        });
    }
    let mut open = Vec::new();
    while let Some(r) = probes.join_next().await {
        if let Ok(Some(ip)) = r {
            open.push(ip);
        }
    }

    // 2. Confirm they are Moonraker and get a name.
    let Ok(http) = reqwest::Client::builder().timeout(HTTP_TIMEOUT).build() else { return Vec::new() };
    let mut checks = JoinSet::new();
    for ip in open {
        let http = http.clone();
        checks.spawn(async move { identify(&http, ip).await });
    }
    let mut found = Vec::new();
    while let Some(r) = checks.join_next().await {
        if let Ok(Some(d)) = r {
            found.push(d);
        }
    }
    found
}

async fn identify(http: &reqwest::Client, ip: Ipv4Addr) -> Option<Discovered> {
    let base = format!("http://{ip}:{PORT}");
    let resp = http.get(format!("{base}/printer/info")).send().await.ok()?;
    let status = resp.status();
    if status.as_u16() == 401 || status.as_u16() == 403 {
        // Moonraker with authentication: we can still offer it.
        return Some(Discovered {
            name: ip.to_string(),
            url: base,
            hostname: String::new(),
            ip: ip.to_string(),
            source: "scan".into(),
            needs_api_key: true,
        });
    }
    let body: Value = resp.json().await.ok()?;
    let result = body.get("result")?;
    let hostname = result.get("hostname").and_then(Value::as_str).unwrap_or("").to_string();
    // Only Moonraker answers /printer/info with a Klippy state.
    result.get("state")?;
    Some(Discovered {
        name: if hostname.is_empty() { ip.to_string() } else { hostname.clone() },
        url: base,
        hostname,
        ip: ip.to_string(),
        source: "scan".into(),
        needs_api_key: false,
    })
}

/// Fetches a file from the printer's `gcodes` root (thumbnails) and returns it as a data URL.
pub async fn fetch_gcode_file(base_url: &str, api_key: Option<&str>, path: &str) -> Result<String, String> {
    use base64::Engine as _;
    let encoded: Vec<String> = path
        .split('/')
        .map(|seg| url::form_urlencoded::byte_serialize(seg.as_bytes()).collect::<String>().replace('+', "%20"))
        .collect();
    let url = format!("{}/server/files/gcodes/{}", base_url.trim_end_matches('/'), encoded.join("/"));
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(8))
        .build()
        .map_err(|e| e.to_string())?;
    let mut req = http.get(&url);
    if let Some(key) = api_key.filter(|k| !k.is_empty()) {
        req = req.header("X-Api-Key", key);
    }
    let resp = req.send().await.map_err(|e| format!("Thumbnail: {e}"))?;
    if !resp.status().is_success() {
        return Err(format!("Thumbnail: HTTP {}", resp.status()));
    }
    let mime = resp
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or("image/png")
        .to_string();
    let bytes = resp.bytes().await.map_err(|e| e.to_string())?;
    Ok(format!("data:{mime};base64,{}", base64::engine::general_purpose::STANDARD.encode(&bytes)))
}
