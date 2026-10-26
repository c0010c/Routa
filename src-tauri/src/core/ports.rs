//! Which process owns a TCP LISTEN socket on a port (spike S5: mihomo answers
//! 204 even when a listener failed to bind, so owners must be checked).

use std::collections::HashMap;
use std::sync::Mutex;

pub trait PortOwners: Send + Sync {
    /// For each requested port that has a TCP LISTEN socket (any local
    /// address, IPv4 or IPv6), the owning PID. Ports without one are absent.
    fn listening_owners(&self, ports: &[u16]) -> HashMap<u16, u32>;
}

/// Test double with a fixed port -> pid table.
#[derive(Debug, Default)]
pub struct FakePortOwners(pub Mutex<HashMap<u16, u32>>);

impl PortOwners for FakePortOwners {
    fn listening_owners(&self, ports: &[u16]) -> HashMap<u16, u32> {
        let table = self.0.lock().unwrap();
        ports
            .iter()
            .filter_map(|p| table.get(p).map(|pid| (*p, *pid)))
            .collect()
    }
}

#[cfg(windows)]
#[derive(Debug, Default, Clone, Copy)]
pub struct WindowsPortOwners;

#[cfg(windows)]
impl WindowsPortOwners {
    /// Reads the whole listener table for one address family.
    fn table(family: u32) -> Vec<(u16, u32)> {
        use windows_sys::Win32::NetworkManagement::IpHelper::{
            GetExtendedTcpTable, MIB_TCP6ROW_OWNER_PID, MIB_TCPROW_OWNER_PID,
            TCP_TABLE_OWNER_PID_LISTENER,
        };

        let mut size: u32 = 0;
        let mut buf: Vec<u8> = Vec::new();
        // The table can grow between calls; retry a few times.
        for _ in 0..8 {
            // SAFETY: buf has at least `size` bytes (empty buf with size 0 only
            // asks for the required size).
            let rc = unsafe {
                GetExtendedTcpTable(
                    if buf.is_empty() {
                        std::ptr::null_mut()
                    } else {
                        buf.as_mut_ptr().cast()
                    },
                    &mut size,
                    0,
                    family,
                    TCP_TABLE_OWNER_PID_LISTENER,
                    0,
                )
            };
            match rc {
                0 => {
                    let count = u32::from_ne_bytes(buf[0..4].try_into().unwrap()) as usize;
                    let rows = &buf[4..];
                    let mut out = Vec::with_capacity(count);
                    for i in 0..count {
                        // Rows are packed after the u32 count; read unaligned.
                        if family == 2 {
                            let sz = std::mem::size_of::<MIB_TCPROW_OWNER_PID>();
                            let row: MIB_TCPROW_OWNER_PID = unsafe {
                                std::ptr::read_unaligned(rows[i * sz..].as_ptr().cast())
                            };
                            out.push((u16::from_be(row.dwLocalPort as u16), row.dwOwningPid));
                        } else {
                            let sz = std::mem::size_of::<MIB_TCP6ROW_OWNER_PID>();
                            let row: MIB_TCP6ROW_OWNER_PID = unsafe {
                                std::ptr::read_unaligned(rows[i * sz..].as_ptr().cast())
                            };
                            out.push((u16::from_be(row.dwLocalPort as u16), row.dwOwningPid));
                        }
                    }
                    return out;
                }
                122 => buf = vec![0u8; size as usize], // ERROR_INSUFFICIENT_BUFFER
                _ => return Vec::new(),
            }
        }
        Vec::new()
    }
}

#[cfg(windows)]
impl PortOwners for WindowsPortOwners {
    fn listening_owners(&self, ports: &[u16]) -> HashMap<u16, u32> {
        const AF_INET: u32 = 2;
        const AF_INET6: u32 = 23;
        let mut out = HashMap::new();
        for family in [AF_INET, AF_INET6] {
            for (port, pid) in Self::table(family) {
                if ports.contains(&port) {
                    out.entry(port).or_insert(pid);
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_returns_only_requested_known_ports() {
        let fake = FakePortOwners(Mutex::new(HashMap::from([(1, 10), (2, 20)])));
        let got = fake.listening_owners(&[2, 3]);
        assert_eq!(got, HashMap::from([(2, 20)]));
    }

    #[cfg(windows)]
    #[test]
    #[ignore = "binds a real socket and reads the system TCP table"]
    fn real_port_owner_matches_own_listener() {
        let l = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = l.local_addr().unwrap().port();
        let owners = WindowsPortOwners.listening_owners(&[port]);
        assert_eq!(owners.get(&port), Some(&std::process::id()));
    }
}
