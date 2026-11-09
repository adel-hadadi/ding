//! Finding other Ding apps on the local network.
//!
//! We don't scan IP addresses. Each app announces a small mDNS record (the same idea as
//! AirPlay or a printer), and everyone else listens. A network that isolates Wi-Fi clients
//! from each other will hide them; that's a router setting, not something Ding can bypass.

use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket};

use ding_shared::{normalize_name, normalize_team};
use mdns_sd::{ServiceDaemon, ServiceEvent, ServiceInfo};
use tokio::sync::mpsc;

pub const SERVICE_TYPE: &str = "_ding._tcp.local.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Peer {
    pub fullname: String,
    pub name: String,
    pub id: String,
    pub team: Option<String>,
    /// True when this peer is the one hosting `team`.
    pub hosting: bool,
    pub addr: SocketAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LanEvent {
    Up(Peer),
    Down { fullname: String },
}

pub struct Lan {
    daemon: ServiceDaemon,
    instance: String,
    hostname: String,
    ip: String,
    port: u16,
    name: String,
    id: String,
}

impl Lan {
    pub fn start(
        name: &str,
        port: u16,
        events: mpsc::UnboundedSender<LanEvent>,
    ) -> Result<Self, String> {
        let id = random_id();
        let ip = primary_ipv4();
        let instance = format!("ding-{id}");
        let hostname = format!("ding-{id}.local.");
        let daemon = ServiceDaemon::new().map_err(|err| err.to_string())?;
        let info = service_info(&Announcement {
            instance: &instance,
            hostname: &hostname,
            ip: &ip,
            port,
            name,
            id: &id,
            team: None,
            hosting: false,
        })?;
        let fullname = info.get_fullname().to_string();
        daemon.register(info).map_err(|err| err.to_string())?;

        let receiver = match daemon.browse(SERVICE_TYPE) {
            Ok(receiver) => receiver,
            Err(err) => {
                let _ = daemon.shutdown();
                return Err(err.to_string());
            }
        };
        let my_id = id.clone();
        // The daemon thread does the network I/O. This task just turns its events into
        // the same channel the UI loop already reads.
        tokio::spawn(async move {
            while let Ok(event) = receiver.recv_async().await {
                match event {
                    ServiceEvent::ServiceResolved(resolved) => {
                        if resolved.fullname == fullname {
                            continue;
                        }
                        let Some(peer) = peer_from(&resolved) else {
                            continue;
                        };
                        if peer.id == my_id {
                            continue;
                        }
                        if events.send(LanEvent::Up(peer)).is_err() {
                            break;
                        }
                    }
                    ServiceEvent::ServiceRemoved(_, gone) => {
                        if gone == fullname {
                            continue;
                        }
                        if events.send(LanEvent::Down { fullname: gone }).is_err() {
                            break;
                        }
                    }
                    _ => {}
                }
            }
        });

        Ok(Self {
            daemon,
            instance,
            hostname,
            ip,
            port,
            name: name.to_string(),
            id,
        })
    }

    /// Update the announcement when we create, join, or leave a team.
    /// Registering the same instance again replaces the old record.
    pub fn set_team(&self, team: Option<&str>, hosting: bool) -> Result<(), String> {
        let info = service_info(&Announcement {
            instance: &self.instance,
            hostname: &self.hostname,
            ip: &self.ip,
            port: self.port,
            name: &self.name,
            id: &self.id,
            team,
            hosting,
        })?;
        self.daemon.register(info).map_err(|err| err.to_string())
    }

    pub fn shutdown(&self) {
        let _ = self.daemon.shutdown();
    }
}

struct Announcement<'a> {
    instance: &'a str,
    hostname: &'a str,
    ip: &'a str,
    port: u16,
    name: &'a str,
    id: &'a str,
    team: Option<&'a str>,
    hosting: bool,
}

fn service_info(announce: &Announcement<'_>) -> Result<ServiceInfo, String> {
    let team = announce.team.unwrap_or("");
    let hosting = if announce.hosting && !team.is_empty() {
        "1"
    } else {
        "0"
    };
    let properties = [
        ("name", announce.name),
        ("id", announce.id),
        ("team", team),
        ("hosting", hosting),
    ];
    // Advertise only the default-route address. Auto-detected addresses also include
    // virtual interfaces (VPN, bridges) that teammates often cannot route to.
    ServiceInfo::new(
        SERVICE_TYPE,
        announce.instance,
        announce.hostname,
        announce.ip,
        announce.port,
        &properties[..],
    )
    .map_err(|err| err.to_string())
}

fn peer_from(service: &mdns_sd::ResolvedService) -> Option<Peer> {
    if !service.is_valid() || service.port == 0 {
        return None;
    }
    let name = normalize_name(service.txt_properties.get_property_val_str("name")?).ok()?;
    let id = service
        .txt_properties
        .get_property_val_str("id")?
        .trim()
        .to_string();
    if id.is_empty() {
        return None;
    }
    let team = service
        .txt_properties
        .get_property_val_str("team")
        .filter(|team| !team.is_empty())
        .and_then(|team| normalize_team(team).ok());
    let hosting =
        service.txt_properties.get_property_val_str("hosting") == Some("1") && team.is_some();
    let ip = pick_ip(&service.addresses)?;
    Some(Peer {
        fullname: service.fullname.clone(),
        name,
        id,
        team,
        hosting,
        addr: SocketAddr::new(ip, service.port),
    })
}

/// Prefer a normal IPv4 address. Loopback is only useful for two apps on one computer
/// that has no other network, and it must not win when a real LAN address is present.
pub fn describe_note(peer: &Peer) -> String {
    match (&peer.team, peer.hosting) {
        (Some(team), true) => format!("hosting \"{team}\""),
        (Some(team), false) => format!("in \"{team}\""),
        _ => "not in a team".into(),
    }
}

pub fn pick_ip(addresses: &HashSet<mdns_sd::ScopedIp>) -> Option<IpAddr> {
    let mut best: Option<IpAddr> = None;
    let mut best_rank = u8::MAX;
    for scoped in addresses {
        let ip = scoped.to_ip_addr();
        let rank = rank_ip(&ip);
        if rank < best_rank {
            best = Some(ip);
            best_rank = rank;
        }
    }
    best
}

fn rank_ip(ip: &IpAddr) -> u8 {
    match ip {
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_link_local() && !ip.is_unspecified() => 0,
        IpAddr::V4(ip) if !ip.is_loopback() && !ip.is_unspecified() => 1,
        IpAddr::V6(ip) if !ip.is_loopback() && !ip.is_unspecified() => 2,
        IpAddr::V4(ip) if ip.is_loopback() => 3,
        _ => 4,
    }
}

/// The address we'd use to reach the internet, which is the LAN address of the default route.
/// UDP `connect` doesn't send a packet; it just asks the OS which interface it would use.
pub fn primary_ipv4() -> String {
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                if let IpAddr::V4(ip) = addr.ip() {
                    if !ip.is_unspecified() && !ip.is_loopback() {
                        return ip.to_string();
                    }
                }
            }
        }
    }
    Ipv4Addr::LOCALHOST.to_string()
}

fn random_id() -> String {
    use std::collections::hash_map::RandomState;
    use std::hash::{BuildHasher, Hasher};
    use std::time::{SystemTime, UNIX_EPOCH};

    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    let mut hasher = RandomState::new().build_hasher();
    hasher.write_u64(nanos);
    hasher.write(b"ding");
    format!("{:08x}", hasher.finish() as u32)
}

#[cfg(test)]
mod tests {
    use super::*;
    use mdns_sd::ScopedIp;
    use std::net::Ipv6Addr;

    #[test]
    fn lan_address_beats_loopback() {
        let addresses = HashSet::from([
            ScopedIp::from(IpAddr::V4(Ipv4Addr::LOCALHOST)),
            ScopedIp::from(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20))),
            ScopedIp::from(IpAddr::V4(Ipv4Addr::new(169, 254, 1, 1))),
            ScopedIp::from(IpAddr::V6(Ipv6Addr::LOCALHOST)),
        ]);
        assert_eq!(
            pick_ip(&addresses),
            Some(IpAddr::V4(Ipv4Addr::new(192, 168, 1, 20)))
        );
    }
}
