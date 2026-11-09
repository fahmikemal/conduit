//! Client address resolution behind trusted proxies, following nginx's realip module.

use std::net::IpAddr;

use http::HeaderName;
use http::HeaderValue;

/// Cloudflare's published ranges (https://www.cloudflare.com/ips/).
pub const CLOUDFLARE: &[&str] = &[
    "173.245.48.0/20",
    "103.21.244.0/22",
    "103.22.200.0/22",
    "103.31.4.0/22",
    "141.101.64.0/18",
    "108.162.192.0/18",
    "190.93.240.0/20",
    "188.114.96.0/20",
    "197.234.240.0/22",
    "198.41.128.0/17",
    "162.158.0.0/15",
    "104.16.0.0/13",
    "104.24.0.0/14",
    "172.64.0.0/13",
    "131.0.72.0/22",
    "2400:cb00::/32",
    "2606:4700::/32",
    "2803:f800::/32",
    "2405:b500::/32",
    "2405:8100::/32",
    "2a06:98c0::/29",
    "2c0f:f248::/32",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Cidr {
    net: IpAddr,
    prefix: u8,
}

impl Cidr {
    pub fn parse(s: &str) -> Result<Cidr, String> {
        let s = s.trim();
        let (addr, prefix) = match s.split_once('/') {
            Some((a, p)) => (a, Some(p)),
            None => (s, None),
        };
        let net: IpAddr = addr
            .parse()
            .map_err(|_| format!("invalid address in {s:?}"))?;
        let max = if net.is_ipv4() { 32 } else { 128 };
        let prefix = match prefix {
            Some(p) => p
                .parse::<u8>()
                .ok()
                .filter(|p| *p <= max)
                .ok_or_else(|| format!("invalid prefix in {s:?}"))?,
            None => max,
        };
        Ok(Cidr { net, prefix })
    }

    pub fn contains(&self, ip: IpAddr) -> bool {
        match (self.net, canonical(ip)) {
            (IpAddr::V4(n), IpAddr::V4(a)) => {
                let mask = u32::MAX
                    .checked_shl(32 - u32::from(self.prefix))
                    .unwrap_or(0);
                u32::from(n) & mask == u32::from(a) & mask
            }
            (IpAddr::V6(n), IpAddr::V6(a)) => {
                let mask = u128::MAX
                    .checked_shl(128 - u32::from(self.prefix))
                    .unwrap_or(0);
                u128::from(n) & mask == u128::from(a) & mask
            }
            _ => false,
        }
    }
}

/// IPv4-mapped IPv6 (`::ffff:1.2.3.4`, as seen on dual-stack sockets) compares as IPv4.
fn canonical(ip: IpAddr) -> IpAddr {
    match ip {
        IpAddr::V6(v6) => v6.to_ipv4_mapped().map_or(ip, IpAddr::V4),
        v4 => v4,
    }
}

#[derive(Clone, Debug)]
pub struct RealIp {
    pub header: HeaderName,
    trusted: Vec<Cidr>,
    recursive: bool,
}

impl RealIp {
    pub fn new(header: &str, trusted: &[String], recursive: bool) -> Result<RealIp, String> {
        let header = HeaderName::try_from(header)
            .map_err(|_| format!("invalid real_ip header {header:?}"))?;
        let mut cidrs = Vec::new();
        for t in trusted {
            if t.eq_ignore_ascii_case("cloudflare") {
                for c in CLOUDFLARE {
                    cidrs.push(Cidr::parse(c)?);
                }
            } else {
                cidrs.push(Cidr::parse(t)?);
            }
        }
        Ok(RealIp {
            header,
            trusted: cidrs,
            recursive,
        })
    }

    pub fn is_trusted(&self, ip: IpAddr) -> bool {
        self.trusted.iter().any(|c| c.contains(ip))
    }

    /// The client address and whether the connecting peer is a trusted proxy.
    pub fn resolve(&self, peer: IpAddr, value: Option<&HeaderValue>) -> (IpAddr, bool) {
        let peer = canonical(peer);
        if !self.is_trusted(peer) {
            return (peer, false);
        }
        let Some(list) = value.and_then(|v| v.to_str().ok()) else {
            return (peer, true);
        };
        let mut candidates = list.rsplit(',').map(str::trim).filter(|s| !s.is_empty());
        if !self.recursive {
            let last = candidates.next().and_then(|s| s.parse::<IpAddr>().ok());
            return (last.map_or(peer, canonical), true);
        }
        let mut chosen = peer;
        for candidate in candidates {
            let Ok(ip) = candidate.parse::<IpAddr>() else {
                break;
            };
            chosen = canonical(ip);
            if !self.is_trusted(chosen) {
                break;
            }
        }
        (chosen, true)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn cidr_contains() {
        let c = Cidr::parse("10.0.0.0/8").unwrap();
        assert!(c.contains(ip("10.20.30.140")));
        assert!(!c.contains(ip("11.0.0.1")));
        assert!(c.contains(ip("::ffff:10.1.2.3")));
        let all = Cidr::parse("0.0.0.0/0").unwrap();
        assert!(all.contains(ip("8.8.8.8")));
        let v6 = Cidr::parse("2606:4700::/32").unwrap();
        assert!(v6.contains(ip("2606:4700:10::1")));
        assert!(!v6.contains(ip("2606:4701::1")));
        assert!(Cidr::parse("10.0.0.0/33").is_err());
    }

    #[test]
    fn untrusted_peer_header_ignored() {
        let r = RealIp::new("CF-Connecting-IP", &["cloudflare".into()], true).unwrap();
        let spoof = HeaderValue::from_static("1.2.3.4");
        assert_eq!(
            r.resolve(ip("203.0.113.9"), Some(&spoof)),
            (ip("203.0.113.9"), false)
        );
    }

    #[test]
    fn trusted_peer_header_used() {
        let r = RealIp::new("CF-Connecting-IP", &["cloudflare".into()], true).unwrap();
        let v = HeaderValue::from_static("114.127.245.14");
        assert_eq!(
            r.resolve(ip("104.22.224.16"), Some(&v)),
            (ip("114.127.245.14"), true)
        );
        assert_eq!(
            r.resolve(ip("104.22.224.16"), None),
            (ip("104.22.224.16"), true)
        );
    }

    #[test]
    fn recursive_skips_trusted_hops() {
        let r = RealIp::new("X-Forwarded-For", &["10.0.0.0/8".into()], true).unwrap();
        let v = HeaderValue::from_static("6.6.6.6, 1.1.1.1, 10.0.0.2");
        assert_eq!(r.resolve(ip("10.0.0.1"), Some(&v)).0, ip("1.1.1.1"));
        let nr = RealIp::new("X-Forwarded-For", &["10.0.0.0/8".into()], false).unwrap();
        assert_eq!(nr.resolve(ip("10.0.0.1"), Some(&v)).0, ip("10.0.0.2"));
        let junk = HeaderValue::from_static("not-an-ip");
        assert_eq!(r.resolve(ip("10.0.0.1"), Some(&junk)).0, ip("10.0.0.1"));
    }
}
