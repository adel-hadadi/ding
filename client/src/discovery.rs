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
    pub headphones: bool,
    /// Encoded blob avatar, when the peer advertised one.
    pub avatar: Option<String>,
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
    team: Option<String>,
    hosting: bool,
    headphones: bool,
    avatar: String,
}

pub struct Presence {
    pub headphones: bool,
    pub avatar: String,
}

impl Lan {
    pub fn start(
        name: &str,
        port: u16,
        presence: Presence,
        events: mpsc::UnboundedSender<LanEvent>,
    ) -> Result<Self, String> {
        let id = random_id();
        let ip = primary_ipv4();
        let instance = format!("ding-{id}");
        let hostname = format!("ding-{id}.local.");
        let daemon = ServiceDaemon::new().map_err(|err| err.to_string())?;
        let lan = Self {
            daemon,
            instance,
            hostname,
            ip,
            port,
            name: name.to_string(),
            id,
            team: None,
            hosting: false,
            headphones: presence.headphones,
            avatar: presence.avatar,
        };
        let info = lan.service_info()?;
        let fullname = info.get_fullname().to_string();
        lan.daemon.register(info).map_err(|err| err.to_string())?;

        let receiver = match lan.daemon.browse(SERVICE_TYPE) {
            Ok(receiver) => receiver,
            Err(err) => {
                let _ = lan.daemon.shutdown();
                return Err(err.to_string());
            }
        };
        let my_id = lan.id.clone();
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

        Ok(lan)
    }

    /// Update the announcement when we create, join, or leave a team.
    /// Registering the same instance again replaces the old record.
    pub fn set_team(&mut self, team: Option<&str>, hosting: bool) -> Result<(), String> {
        self.team = team
            .filter(|team| !team.is_empty())
            .map(|team| team.to_string());
        self.hosting = hosting && self.team.is_some();
        self.reregister()
    }

    /// Update headphones / avatar without changing the team announcement.
    pub fn set_presence(&mut self, headphones: bool, avatar: String) -> Result<(), String> {
        self.headphones = headphones;
        self.avatar = avatar;
        self.reregister()
    }

    fn reregister(&mut self) -> Result<(), String> {
        let info = self.service_info()?;
        self.daemon.register(info).map_err(|err| err.to_string())
    }

    fn service_info(&self) -> Result<ServiceInfo, String> {
        let team = self.team.as_deref().unwrap_or("");
        let hosting = if self.hosting && !team.is_empty() {
            "1"
        } else {
            "0"
        };
        let hp = if self.headphones { "1" } else { "0" };
        let properties = [
            ("name", self.name.as_str()),
            ("id", self.id.as_str()),
            ("team", team),
            ("hosting", hosting),
            ("hp", hp),
            ("avatar", self.avatar.as_str()),
        ];
        // Advertise only the default-route address. Auto-detected addresses also include
        // virtual interfaces (VPN, bridges) that teammates often cannot route to.
        ServiceInfo::new(
            SERVICE_TYPE,
            &self.instance,
            &self.hostname,
            self.ip.as_str(),
            self.port,
            &properties[..],
        )
        .map_err(|err| err.to_string())
    }

    pub fn shutdown(&self) {
        let _ = self.daemon.shutdown();
    }
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
    let headphones = service.txt_properties.get_property_val_str("hp") == Some("1");
    let avatar = service
        .txt_properties
        .get_property_val_str("avatar")
        .map(str::trim)
        .filter(|avatar| !avatar.is_empty())
        .map(str::to_string);
    let ip = pick_ip(&service.addresses)?;
    Some(Peer {
        fullname: service.fullname.clone(),
        name,
        id,
        team,
        hosting,
        headphones,
        avatar,
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
