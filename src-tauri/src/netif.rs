//! Windows network adapter enumeration.

use std::net::{Ipv4Addr, Ipv6Addr};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};

use crate::model::InterfaceGuid;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, specta::Type)]
pub struct NetInterface {
    /// Adapter GUID exactly as Windows reports it, e.g. `{XXXXXXXX-...}`.
    pub guid: InterfaceGuid,
    pub friendly_name: String,
    pub description: String,
    pub is_up: bool,
    pub ipv4: Vec<Ipv4Addr>,
    pub ipv6: Vec<Ipv6Addr>,
    /// Value for mihomo's `interface-name`: the friendly name verbatim,
    /// case preserved (spike S1: GUID and description do not work).
    pub mihomo_name: String,
}

impl NetInterface {
    /// `is_up` and at least one routable address: IPv4/IPv6 loopback and
    /// link-local (169.254.0.0/16, fe80::/10) addresses do not count.
    ///
    /// This is a working hypothesis, not a proven rule. Spike S1: an Up adapter
    /// with no IP at all (`WLAN 2`) fails in mihomo with "interface not found",
    /// while adapters with addresses were found; IPv6-only / IPv4-only adapters
    /// were not tested. Spike S2: a missing or disconnected adapter makes the
    /// dial fail with no fallback to the default NIC, so unusable means the
    /// exit must stay disabled.
    pub fn is_usable(&self) -> bool {
        self.is_up
            && (self.ipv4.iter().copied().any(is_routable_v4)
                || self.ipv6.iter().copied().any(is_routable_v6))
    }
}

fn is_routable_v4(a: Ipv4Addr) -> bool {
    !a.is_loopback() && !a.is_link_local()
}

fn is_routable_v6(a: Ipv6Addr) -> bool {
    !a.is_loopback() && (a.segments()[0] & 0xffc0) != 0xfe80
}

pub trait InterfaceSource: Send + Sync {
    fn list(&self) -> Vec<NetInterface>;
}

/// In-memory source for tests.
pub struct FakeInterfaces(pub Mutex<Vec<NetInterface>>);

impl InterfaceSource for FakeInterfaces {
    fn list(&self) -> Vec<NetInterface> {
        self.0.lock().unwrap().clone()
    }
}

/// Real adapters via the Win32 adapter list (`ipconfig` crate).
#[cfg(windows)]
pub struct WindowsInterfaces;

#[cfg(windows)]
impl InterfaceSource for WindowsInterfaces {
    fn list(&self) -> Vec<NetInterface> {
        use ipconfig::{IfType, OperStatus};
        use std::net::IpAddr;

        let Ok(adapters) = ipconfig::get_adapters() else {
            return Vec::new();
        };
        adapters
            .iter()
            .filter(|a| !matches!(a.if_type(), IfType::SoftwareLoopback | IfType::Tunnel))
            .map(|a| {
                let mut ipv4 = Vec::new();
                let mut ipv6 = Vec::new();
                for ip in a.ip_addresses() {
                    match ip {
                        IpAddr::V4(v) if is_routable_v4(*v) => ipv4.push(*v),
                        IpAddr::V6(v) if is_routable_v6(*v) => ipv6.push(*v),
                        _ => {}
                    }
                }
                NetInterface {
                    guid: InterfaceGuid(a.adapter_name().to_string()),
                    friendly_name: a.friendly_name().to_string(),
                    description: a.description().to_string(),
                    is_up: a.oper_status() == OperStatus::IfOperStatusUp,
                    ipv4,
                    ipv6,
                    mihomo_name: a.friendly_name().to_string(),
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn nic(is_up: bool, ipv4: &[Ipv4Addr], ipv6: &[Ipv6Addr]) -> NetInterface {
        NetInterface {
            guid: InterfaceGuid("{00000000-0000-0000-0000-000000000001}".into()),
            friendly_name: "Ethernet".into(),
            description: "Test Adapter".into(),
            is_up,
            ipv4: ipv4.to_vec(),
            ipv6: ipv6.to_vec(),
            mihomo_name: "Ethernet".into(),
        }
    }

    #[test]
    fn usable_requires_up_and_address() {
        let v4 = [Ipv4Addr::new(192, 168, 1, 2)];
        let v6 = [Ipv6Addr::new(0x2001, 0xdb8, 0, 0, 0, 0, 0, 1)];
        assert!(!nic(true, &[], &[]).is_usable());
        assert!(!nic(false, &v4, &[]).is_usable());
        assert!(nic(true, &v4, &[]).is_usable());
        assert!(nic(true, &[], &v6).is_usable());
    }

    #[test]
    fn link_local_only_is_not_usable() {
        let ll4 = Ipv4Addr::new(169, 254, 1, 2);
        let ll6 = Ipv6Addr::new(0xfe80, 0, 0, 0, 0, 0, 0, 1);
        assert!(!nic(true, &[ll4], &[]).is_usable());
        assert!(!nic(true, &[], &[ll6]).is_usable());
        assert!(nic(true, &[ll4, Ipv4Addr::new(192, 168, 1, 5)], &[]).is_usable());
    }

    #[test]
    fn fake_source_returns_contents() {
        let n = nic(true, &[Ipv4Addr::new(10, 0, 0, 1)], &[]);
        let src = FakeInterfaces(Mutex::new(vec![n.clone()]));
        assert_eq!(src.list(), vec![n]);
    }

    /// Manual: `cargo test netif:: -- --ignored --nocapture`, compare with Get-NetAdapter.
    #[test]
    #[ignore]
    #[cfg(windows)]
    fn lists_real_adapters() {
        let list = WindowsInterfaces.list();
        for n in &list {
            println!(
                "{:<24} up={:<5} usable={:<5} guid={} v4={:?} v6={:?} desc={}",
                n.mihomo_name, n.is_up, n.is_usable(), n.guid.0, n.ipv4, n.ipv6, n.description
            );
        }
        assert!(!list.is_empty());
    }
}
