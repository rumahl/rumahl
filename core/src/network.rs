//! Network target classification for brokered app requests.
//!
//! The transport always blocks special-use destinations (unspecified,
//! broadcast, multicast, link-local). Private and loopback targets additionally
//! require an explicit private-network consent on the origin grant; a public
//! grant must never reach them through DNS.

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Destinations that are never reachable, even with private consent.
pub fn blocked(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => blocked_v4(ip),
        IpAddr::V6(ip) => blocked_v6(ip),
    }
}

/// Destinations that need the explicit private-network consent.
pub fn private(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(ip) => private_v4(ip),
        IpAddr::V6(ip) => {
            // Check loopback before `to_ipv4`: `::1` is also the deprecated
            // IPv4-compatible address 0.0.0.1, which is not IPv4 loopback.
            if ip.is_loopback() {
                return true;
            }
            if let Some(v4) = ip.to_ipv4() {
                return private_v4(v4);
            }
            let segments = ip.segments();
            (segments[0] & 0xfe00) == 0xfc00
        }
    }
}

/// True when a brokered request may target this address under the grant scope.
pub fn allowed(ip: IpAddr, allow_private: bool) -> bool {
    !blocked(ip) && (allow_private || !private(ip))
}

fn blocked_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_link_local()
        || ip.is_unspecified()
        || ip.is_broadcast()
        || ip.is_multicast()
        || octets[0] == 0
}

fn blocked_v6(ip: Ipv6Addr) -> bool {
    ip.is_unspecified() || ip.is_multicast() || (ip.segments()[0] & 0xffc0) == 0xfe80
}

fn private_v4(ip: Ipv4Addr) -> bool {
    let octets = ip.octets();
    ip.is_loopback() || ip.is_private() || (octets[0] == 100 && (64..=127).contains(&octets[1]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn special_use_targets_stay_blocked_with_private_consent() {
        for value in [
            "169.254.169.254",
            "fe80::1",
            "0.0.0.0",
            "0.1.2.3",
            "224.0.0.1",
            "ff02::1",
        ] {
            let ip: IpAddr = value.parse().unwrap();
            assert!(blocked(ip), "{value} must be blocked");
            assert!(!allowed(ip, true), "{value} must not be reachable");
        }
    }

    #[test]
    fn private_and_loopback_need_private_consent() {
        for value in [
            "127.0.0.1",
            "::1",
            "192.168.1.2",
            "10.0.0.1",
            "fd00::1",
            "100.64.0.1",
        ] {
            let ip: IpAddr = value.parse().unwrap();
            assert!(private(ip), "{value} must be private");
            assert!(!allowed(ip, false), "{value} must need consent");
            assert!(allowed(ip, true), "{value} must be reachable with consent");
        }
        for value in ["93.184.216.34", "2606:4700:4700::1111", "2001:db8::1"] {
            let ip: IpAddr = value.parse().unwrap();
            assert!(!private(ip), "{value} must be public");
            assert!(allowed(ip, false), "{value} must be reachable");
        }
    }
}
