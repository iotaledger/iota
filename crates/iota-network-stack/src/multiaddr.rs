// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    borrow::Cow,
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr},
};

pub use ::multiaddr::{Error, Protocol};
use eyre::{Result, eyre};
use tracing::error;

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct Multiaddr(::multiaddr::Multiaddr);

impl Multiaddr {
    pub fn empty() -> Self {
        Self(::multiaddr::Multiaddr::empty())
    }

    #[cfg(test)]
    pub(crate) fn new_internal(inner: ::multiaddr::Multiaddr) -> Self {
        Self(inner)
    }

    pub fn iter(&self) -> ::multiaddr::Iter<'_> {
        self.0.iter()
    }

    pub fn pop<'a>(&mut self) -> Option<Protocol<'a>> {
        self.0.pop()
    }

    pub fn push(&mut self, p: Protocol<'_>) {
        self.0.push(p)
    }

    pub fn replace<'a, F>(&self, at: usize, by: F) -> Option<Multiaddr>
    where
        F: FnOnce(&Protocol<'_>) -> Option<Protocol<'a>>,
    {
        self.0.replace(at, by).map(Self)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Attempts to convert a multiaddr of the form
    /// `/[ip4,ip6,dns]/{}/udp/{port}` into an anemo address
    pub fn to_anemo_address(&self) -> Result<anemo::types::Address, &'static str> {
        let mut iter = self.iter();

        match (iter.next(), iter.next()) {
            (Some(Protocol::Ip4(ipaddr)), Some(Protocol::Udp(port))) => Ok((ipaddr, port).into()),
            (Some(Protocol::Ip6(ipaddr)), Some(Protocol::Udp(port))) => Ok((ipaddr, port).into()),
            (Some(Protocol::Dns(hostname)), Some(Protocol::Udp(port))) => {
                Ok((hostname.as_ref(), port).into())
            }

            _ => {
                tracing::warn!("unsupported p2p multiaddr: '{self}'");
                Err("invalid address")
            }
        }
    }

    pub fn udp_multiaddr_to_listen_address(&self) -> Option<std::net::SocketAddr> {
        let mut iter = self.iter();

        match (iter.next(), iter.next()) {
            (Some(Protocol::Ip4(ipaddr)), Some(Protocol::Udp(port))) => Some((ipaddr, port).into()),
            (Some(Protocol::Ip6(ipaddr)), Some(Protocol::Udp(port))) => Some((ipaddr, port).into()),

            (Some(Protocol::Dns(_)), Some(Protocol::Udp(port))) => {
                Some((std::net::Ipv4Addr::UNSPECIFIED, port).into())
            }

            _ => None,
        }
    }

    // Converts a /ip{4,6}/-/tcp/-[/-] Multiaddr to SocketAddr.
    // Useful when an external library only accepts SocketAddr, e.g. to start a
    // local server. See `client::endpoint_from_multiaddr()` for converting to
    // Endpoint for clients.
    pub fn to_socket_addr(&self) -> Result<SocketAddr> {
        let mut iter = self.iter();
        let ip = match iter.next().ok_or_else(|| {
            eyre!("failed to convert to SocketAddr: Multiaddr does not contain IP")
        })? {
            Protocol::Ip4(ip4_addr) => IpAddr::V4(ip4_addr),
            Protocol::Ip6(ip6_addr) => IpAddr::V6(ip6_addr),
            unsupported => return Err(eyre!("unsupported protocol {unsupported}")),
        };
        let tcp_port = parse_tcp(&mut iter)?;
        Ok(SocketAddr::new(ip, tcp_port))
    }

    // Returns true if the third component in the multiaddr is `Protocol::Tcp`
    pub fn is_loosely_valid_tcp_addr(&self) -> bool {
        let mut iter = self.iter();
        iter.next(); // Skip the ip/dns part
        match iter.next() {
            Some(Protocol::Tcp(_)) => true,
            _ => false, // including `None` and `Some(other)`
        }
    }

    /// Set the ip address to `0.0.0.0`. For instance, it converts the following
    /// address `/ip4/155.138.174.208/tcp/1500/http` into
    /// `/ip4/0.0.0.0/tcp/1500/http`. This is useful when starting a server
    /// and you want to listen on all interfaces.
    pub fn with_zero_ip(&self) -> Self {
        let mut new_address = self.0.clone();
        let Some(protocol) = new_address.iter().next() else {
            error!("Multiaddr is empty");
            return Self(new_address);
        };
        match protocol {
            multiaddr::Protocol::Ip4(_)
            | multiaddr::Protocol::Dns(_)
            | multiaddr::Protocol::Dns4(_) => {
                new_address = new_address
                    .replace(0, |_| Some(multiaddr::Protocol::Ip4(Ipv4Addr::UNSPECIFIED)))
                    .unwrap();
            }
            multiaddr::Protocol::Ip6(_) | multiaddr::Protocol::Dns6(_) => {
                new_address = new_address
                    .replace(0, |_| Some(multiaddr::Protocol::Ip6(Ipv6Addr::UNSPECIFIED)))
                    .unwrap();
            }
            p => {
                error!("Unsupported protocol {} in Multiaddr {}!", p, new_address);
            }
        }
        Self(new_address)
    }

    /// Set the ip address to `127.0.0.1`. For instance, it converts the
    /// following address `/ip4/155.138.174.208/tcp/1500/http` into
    /// `/ip4/127.0.0.1/tcp/1500/http`.
    pub fn with_localhost_ip(&self) -> Self {
        let mut new_address = self.0.clone();
        let Some(protocol) = new_address.iter().next() else {
            error!("Multiaddr is empty");
            return Self(new_address);
        };
        match protocol {
            multiaddr::Protocol::Ip4(_)
            | multiaddr::Protocol::Dns(_)
            | multiaddr::Protocol::Dns4(_) => {
                new_address = new_address
                    .replace(0, |_| Some(multiaddr::Protocol::Ip4(Ipv4Addr::LOCALHOST)))
                    .unwrap();
            }
            multiaddr::Protocol::Ip6(_) | multiaddr::Protocol::Dns6(_) => {
                new_address = new_address
                    .replace(0, |_| Some(multiaddr::Protocol::Ip6(Ipv6Addr::LOCALHOST)))
                    .unwrap();
            }
            p => {
                error!("Unsupported protocol {} in Multiaddr {}!", p, new_address);
            }
        }
        Self(new_address)
    }

    pub fn is_localhost_ip(&self) -> bool {
        let Some(protocol) = self.0.iter().next() else {
            error!("Multiaddr is empty");
            return false;
        };
        match protocol {
            multiaddr::Protocol::Ip4(addr) => addr == Ipv4Addr::LOCALHOST,
            multiaddr::Protocol::Ip6(addr) => addr == Ipv6Addr::LOCALHOST,
            _ => false,
        }
    }

    pub fn hostname(&self) -> Option<String> {
        for component in self.iter() {
            match component {
                Protocol::Ip4(ip) => return Some(ip.to_string()),
                Protocol::Ip6(ip) => return Some(ip.to_string()),
                Protocol::Dns(dns) => return Some(dns.to_string()),
                _ => (),
            }
        }
        None
    }

    pub fn port(&self) -> Option<u16> {
        for component in self.iter() {
            match component {
                Protocol::Udp(port) | Protocol::Tcp(port) => return Some(port),
                _ => (),
            }
        }
        None
    }

    pub fn rewrite_udp_to_tcp(&self) -> Self {
        let mut new = Self::empty();

        for component in self.iter() {
            if let Protocol::Udp(port) = component {
                new.push(Protocol::Tcp(port));
            } else {
                new.push(component);
            }
        }

        new
    }

    /// Checks if the multiaddr contains a private/unroutable IP address or
    /// invalid DNS. Returns true if the address should is private or
    /// unroutable.
    pub fn is_private_or_unroutable(&self, allow_private_addresses: bool) -> bool {
        let Some(protocol) = self.0.iter().next() else {
            return true; // Empty address is not routable
        };

        match protocol {
            multiaddr::Protocol::Ip4(addr) => {
                is_ipv4_private_or_unroutable(addr, allow_private_addresses)
            }
            multiaddr::Protocol::Ip6(addr) => is_ipv6_private_or_unroutable(addr),
            multiaddr::Protocol::Dns(hostname) => {
                !is_valid_fqdn(hostname.as_ref(), allow_private_addresses)
            }
            multiaddr::Protocol::Dns4(hostname) => {
                !is_valid_fqdn(hostname.as_ref(), allow_private_addresses)
            }
            multiaddr::Protocol::Dns6(hostname) => {
                !is_valid_fqdn(hostname.as_ref(), allow_private_addresses)
            }
            _ => true, // Other protocol types are not supported
        }
    }

    /// Checks if the multiaddr is suitable for public announcement for anemo.
    /// This includes checking for private/unroutable addresses and valid
    /// format.
    pub fn is_valid_public_anemo_address(&self, allow_private_addresses: bool) -> bool {
        // Check if address is empty
        if self.is_empty() {
            return false;
        }

        // Check if it can be converted to anemo address (format validation)
        if self.to_anemo_address().is_err() {
            return false;
        }

        // Check if it's a private or unroutable address
        if self.is_private_or_unroutable(allow_private_addresses) {
            return false;
        }

        true
    }
}

impl std::fmt::Display for Multiaddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        std::fmt::Display::fmt(&self.0, f)
    }
}

impl std::str::FromStr for Multiaddr {
    type Err = Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        ::multiaddr::Multiaddr::from_str(s).map(Self)
    }
}

impl<'a> TryFrom<&'a str> for Multiaddr {
    type Error = Error;

    fn try_from(value: &'a str) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl TryFrom<String> for Multiaddr {
    type Error = Error;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        value.parse()
    }
}

impl serde::Serialize for Multiaddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0.to_string())
    }
}

impl<'de> serde::Deserialize<'de> for Multiaddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        s.parse()
            .map(Self)
            .map_err(|e| serde::de::Error::custom(e.to_string()))
    }
}

impl std::net::ToSocketAddrs for Multiaddr {
    type Iter = Box<dyn Iterator<Item = SocketAddr>>;

    fn to_socket_addrs(&self) -> std::io::Result<Self::Iter> {
        let mut iter = self.iter();

        match (iter.next(), iter.next()) {
            (Some(Protocol::Ip4(ip4)), Some(Protocol::Tcp(port) | Protocol::Udp(port))) => {
                (ip4, port)
                    .to_socket_addrs()
                    .map(|iter| Box::new(iter) as _)
            }
            (Some(Protocol::Ip6(ip6)), Some(Protocol::Tcp(port) | Protocol::Udp(port))) => {
                (ip6, port)
                    .to_socket_addrs()
                    .map(|iter| Box::new(iter) as _)
            }
            (Some(Protocol::Dns(hostname)), Some(Protocol::Tcp(port) | Protocol::Udp(port))) => {
                (hostname.as_ref(), port)
                    .to_socket_addrs()
                    .map(|iter| Box::new(iter) as _)
            }
            _ => Err(std::io::Error::new(
                std::io::ErrorKind::InvalidInput,
                "unable to convert Multiaddr to SocketAddr",
            )),
        }
    }
}

pub(crate) fn parse_tcp<'a, T: Iterator<Item = Protocol<'a>>>(protocols: &mut T) -> Result<u16> {
    if let Protocol::Tcp(port) = protocols
        .next()
        .ok_or_else(|| eyre!("unexpected end of multiaddr"))?
    {
        Ok(port)
    } else {
        Err(eyre!("expected tcp protocol"))
    }
}

pub(crate) fn parse_http_https<'a, T: Iterator<Item = Protocol<'a>>>(
    protocols: &mut T,
) -> Result<&'static str> {
    match protocols.next() {
        Some(Protocol::Http) => Ok("http"),
        Some(Protocol::Https) => Ok("https"),
        _ => Ok("http"),
    }
}

pub(crate) fn parse_end<'a, T: Iterator<Item = Protocol<'a>>>(protocols: &mut T) -> Result<()> {
    if protocols.next().is_none() {
        Ok(())
    } else {
        Err(eyre!("expected end of multiaddr"))
    }
}

// Parse a full /dns/-/tcp/-/{http,https} address
pub(crate) fn parse_dns(address: &Multiaddr) -> Result<(Cow<'_, str>, u16, &'static str)> {
    let mut iter = address.iter();

    let dns_name = match iter
        .next()
        .ok_or_else(|| eyre!("unexpected end of multiaddr"))?
    {
        Protocol::Dns(dns_name) => dns_name,
        other => return Err(eyre!("expected dns found {other}")),
    };
    let tcp_port = parse_tcp(&mut iter)?;
    let http_or_https = parse_http_https(&mut iter)?;
    parse_end(&mut iter)?;
    Ok((dns_name, tcp_port, http_or_https))
}

// Parse a full /ip4/-/tcp/-/{http,https} address
pub(crate) fn parse_ip4(address: &Multiaddr) -> Result<(SocketAddr, &'static str)> {
    let mut iter = address.iter();

    let ip_addr = match iter
        .next()
        .ok_or_else(|| eyre!("unexpected end of multiaddr"))?
    {
        Protocol::Ip4(ip4_addr) => IpAddr::V4(ip4_addr),
        other => return Err(eyre!("expected ip4 found {other}")),
    };
    let tcp_port = parse_tcp(&mut iter)?;
    let http_or_https = parse_http_https(&mut iter)?;
    parse_end(&mut iter)?;
    let socket_addr = SocketAddr::new(ip_addr, tcp_port);

    Ok((socket_addr, http_or_https))
}

// Parse a full /ip6/-/tcp/-/{http,https} address
pub(crate) fn parse_ip6(address: &Multiaddr) -> Result<(SocketAddr, &'static str)> {
    let mut iter = address.iter();

    let ip_addr = match iter
        .next()
        .ok_or_else(|| eyre!("unexpected end of multiaddr"))?
    {
        Protocol::Ip6(ip6_addr) => IpAddr::V6(ip6_addr),
        other => return Err(eyre!("expected ip6 found {other}")),
    };
    let tcp_port = parse_tcp(&mut iter)?;
    let http_or_https = parse_http_https(&mut iter)?;
    parse_end(&mut iter)?;
    let socket_addr = SocketAddr::new(ip_addr, tcp_port);

    Ok((socket_addr, http_or_https))
}

/// Checks if an IPv4 address is private, reserved, or otherwise unroutable on
/// the public internet.
fn is_ipv4_private_or_unroutable(addr: Ipv4Addr, allow_private_addresses: bool) -> bool {
    if !allow_private_addresses && addr.is_private() {
        // RFC 1918 - Private Networks (10.0.0.0/8, 172.16.0.0/12, 192.168.0.0/16)
        return true;
    }

    // RFC 1122 - "This" Network (0.0.0.0/8) and Unspecified (0.0.0.0/32)
    addr.is_unspecified() ||
    // RFC 1122 - Loopback (127.0.0.0/8)
    addr.is_loopback() ||
    // RFC 3927 - Link-Local (169.254.0.0/16)
    addr.is_link_local() ||
    // RFC 1112 / RFC 3171 - Multicast (224.0.0.0/4)
    addr.is_multicast() ||
    // RFC 6598 - Shared Address Space / Carrier-Grade NAT (100.64.0.0/10)
    (addr.octets()[0] == 100 && (addr.octets()[1] & 0b11000000) == 64) ||
    // RFC 6890 - IETF Protocol Assignments (192.0.0.0/24)
    (addr.octets()[0] == 192 && addr.octets()[1] == 0 && addr.octets()[2] == 0) ||
    // RFC 5737 - Documentation/TEST-NET addresses (192.0.2.0/24, 198.51.100.0/24, 203.0.113.0/24)
    (addr.octets()[0] == 192 && addr.octets()[1] == 0 && addr.octets()[2] == 2) ||
    (addr.octets()[0] == 198 && addr.octets()[1] == 51 && addr.octets()[2] == 100) ||
    (addr.octets()[0] == 203 && addr.octets()[1] == 0 && addr.octets()[2] == 113) ||
    // RFC 7535 - AS112-v4 (192.31.196.0/24)
    (addr.octets()[0] == 192 && addr.octets()[1] == 31 && addr.octets()[2] == 196) ||
    // RFC 7450 - Automatic Multicast Tunneling (192.52.193.0/24)
    (addr.octets()[0] == 192 && addr.octets()[1] == 52 && addr.octets()[2] == 193) ||
    // RFC 3068 - 6to4 Relay Anycast (192.88.99.0/24)
    (addr.octets()[0] == 192 && addr.octets()[1] == 88 && addr.octets()[2] == 99) ||
    // RFC 2544 - Network Interconnect Device Benchmark Testing (198.18.0.0/15)
    (addr.octets()[0] == 198 && (addr.octets()[1] & 0b11111110) == 18) ||
    // RFC 1112 - Reserved for Future Use (240.0.0.0/4)
    (addr.octets()[0] >= 240)
}

/// Checks if an IPv6 address is private, reserved, or otherwise unroutable on
/// the public internet.
fn is_ipv6_private_or_unroutable(addr: Ipv6Addr) -> bool {
    // RFC 4291 - Unspecified Address (::/128)
    addr.is_unspecified() ||
    // RFC 4291 - Loopback Address (::1/128)
    addr.is_loopback() ||
    // RFC 4291 - Multicast (ff00::/8)
    addr.is_multicast() ||
    // RFC 6666 - Discard-Only Address Block (100::/64)
    (addr.segments()[0] == 0x0100 && addr.segments()[1] == 0 &&
     addr.segments()[2] == 0 && addr.segments()[3] == 0) ||
    // RFC 4380 - Teredo (2001::/32)
    (addr.segments()[0] == 0x2001 && addr.segments()[1] == 0x0000) ||
    // RFC 5180 - Benchmarking (2001:2::/48)
    (addr.segments()[0] == 0x2001 && addr.segments()[1] == 0x0002) ||
    // RFC 4843 - ORCHID (2001:10::/28)
    (addr.segments()[0] == 0x2001 && (addr.segments()[1] & 0xfff0) == 0x0010) ||
    // RFC 3849 - Documentation (2001:db8::/32)
    (addr.segments()[0] == 0x2001 && addr.segments()[1] == 0x0db8) ||
    // RFC 3056 - 6to4 (2002::/16)
    addr.segments()[0] == 0x2002 ||
    // RFC 4193 - Unique Local Addresses (fc00::/7)
    (addr.segments()[0] & 0xfe00) == 0xfc00 ||
    // RFC 3513 - Site-Local (deprecated, fec0::/10)
    (addr.segments()[0] & 0xffc0) == 0xfec0 ||
    // RFC 4862 - Link-Local (fe80::/10)
    (addr.segments()[0] & 0xffc0) == 0xfe80 ||
    // RFC 4291 - IPv4-mapped IPv6 addresses (::ffff:0:0/96) - check embedded IPv4
    addr.to_ipv4_mapped().is_some_and(|addr| is_ipv4_private_or_unroutable(addr, false))
}

/// Checks if a hostname is a valid FQDN (Fully Qualified Domain Name).
/// Returns true if the hostname is valid for DNS resolution.
/// This implements RFC-compliant hostname validation with IDNA support for
/// Unicode domains.
fn is_valid_fqdn(hostname: &str, allow_localhost_dns: bool) -> bool {
    if hostname.ends_with('.') {
        // we do not allow hostnames with dot at the end
        return false;
    }

    // Basic length check
    if hostname.is_empty() || hostname.len() > 253 {
        return false;
    }

    let hostname_lower = hostname.to_lowercase();
    if !allow_localhost_dns {
        // Reject localhost and local domain variants
        if hostname_lower == "localhost" || hostname_lower.ends_with(".local") {
            return false;
        }
    } else if hostname_lower == "localhost" {
        // Skip further checks for exact "localhost"
        return true;
    }

    // Try to convert to ASCII using IDNA (handles Unicode domains)
    let ascii_hostname = match idna::domain_to_ascii(hostname) {
        Ok(ascii) => ascii,
        Err(_) => return false, // Invalid Unicode domain
    };

    // Split into labels
    let labels: Vec<&str> = ascii_hostname.split('.').collect();

    // Must have at least 2 labels (hostname.tld) for public use
    if labels.len() < 2 {
        return false;
    }

    // Validate each label using ASCII rules (after IDNA conversion)
    for label in &labels {
        if !is_valid_dns_label(label) {
            return false;
        }
    }

    // Basic TLD validation: last label should be valid DNS label and >= 2 chars
    // After IDNA conversion, TLDs can be punycode (xn--...) so we use general DNS
    // label validation
    let tld = *labels.last().unwrap();
    if tld.len() < 2 {
        return false;
    }

    // For TLD, we allow punycode (xn--) or pure alphabetic
    let is_valid_tld = if tld.starts_with("xn--") {
        // Punycode TLD - already validated as DNS label above
        true
    } else {
        // Regular TLD - should be alphabetic only
        tld.chars().all(|c| c.is_ascii_alphabetic())
    };

    if !is_valid_tld {
        return false;
    }

    true
}

/// Validates a single DNS label according to RFC 1035
fn is_valid_dns_label(label: &str) -> bool {
    // Label length: 1-63 characters (RFC 1035)
    if label.is_empty() || label.len() > 63 {
        return false;
    }

    let bytes = label.as_bytes();

    // Must start and end with alphanumeric character (no hyphens at boundaries)
    if !bytes[0].is_ascii_alphanumeric() || !bytes[bytes.len() - 1].is_ascii_alphanumeric() {
        return false;
    }

    // Can only contain alphanumeric characters and hyphens
    bytes
        .iter()
        .all(|&b| b.is_ascii_alphanumeric() || b == b'-')
}

#[cfg(test)]
mod test {
    use multiaddr::multiaddr;

    use super::Multiaddr;

    #[test]
    fn test_to_socket_addr_basic() {
        let multi_addr_ipv4 = Multiaddr(multiaddr!(Ip4([127, 0, 0, 1]), Tcp(10500u16)));
        let socket_addr_ipv4 = multi_addr_ipv4
            .to_socket_addr()
            .expect("Couldn't convert to socket addr");
        assert_eq!(socket_addr_ipv4.to_string(), "127.0.0.1:10500");

        let multi_addr_ipv6 = Multiaddr(multiaddr!(Ip6([172, 0, 0, 1, 1, 1, 1, 1]), Tcp(10500u16)));
        let socket_addr_ipv6 = multi_addr_ipv6
            .to_socket_addr()
            .expect("Couldn't convert to socket addr");
        assert_eq!(socket_addr_ipv6.to_string(), "[ac::1:1:1:1:1]:10500");
    }

    #[test]
    fn test_to_socket_addr_unsupported_protocol() {
        let multi_addr_dns = Multiaddr(multiaddr!(Dnsaddr("iota.iota"), Tcp(10500u16)));
        let _ = multi_addr_dns
            .to_socket_addr()
            .expect_err("DNS is unsupported");
    }

    #[test]
    fn test_is_loosely_valid_tcp_addr() {
        let multi_addr_ipv4 = Multiaddr(multiaddr!(Ip4([127, 0, 0, 1]), Tcp(10500u16)));
        assert!(multi_addr_ipv4.is_loosely_valid_tcp_addr());
        let multi_addr_ipv6 = Multiaddr(multiaddr!(Ip6([172, 0, 0, 1, 1, 1, 1, 1]), Tcp(10500u16)));
        assert!(multi_addr_ipv6.is_loosely_valid_tcp_addr());
        let multi_addr_dns = Multiaddr(multiaddr!(Dnsaddr("iota.iota"), Tcp(10500u16)));
        assert!(multi_addr_dns.is_loosely_valid_tcp_addr());

        let multi_addr_ipv4 = Multiaddr(multiaddr!(Ip4([127, 0, 0, 1]), Udp(10500u16)));
        assert!(!multi_addr_ipv4.is_loosely_valid_tcp_addr());
        let multi_addr_ipv6 = Multiaddr(multiaddr!(Ip6([172, 0, 0, 1, 1, 1, 1, 1]), Udp(10500u16)));
        assert!(!multi_addr_ipv6.is_loosely_valid_tcp_addr());
        let multi_addr_dns = Multiaddr(multiaddr!(Dnsaddr("iota.iota"), Udp(10500u16)));
        assert!(!multi_addr_dns.is_loosely_valid_tcp_addr());

        let invalid_multi_addr_ipv4 = Multiaddr(multiaddr!(Ip4([127, 0, 0, 1])));
        assert!(!invalid_multi_addr_ipv4.is_loosely_valid_tcp_addr());
    }

    #[test]
    fn test_get_hostname_port() {
        let multi_addr_ip4 = Multiaddr(multiaddr!(Ip4([127, 0, 0, 1]), Tcp(10500u16)));
        assert_eq!(Some("127.0.0.1".to_string()), multi_addr_ip4.hostname());
        assert_eq!(Some(10500u16), multi_addr_ip4.port());

        let multi_addr_dns = Multiaddr(multiaddr!(Dns("iota.iota"), Tcp(10501u16)));
        assert_eq!(Some("iota.iota".to_string()), multi_addr_dns.hostname());
        assert_eq!(Some(10501u16), multi_addr_dns.port());
    }

    #[test]
    fn test_to_anemo_address() {
        let addr_ip4 = Multiaddr(multiaddr!(Ip4([15, 15, 15, 1]), Udp(10500u16)))
            .to_anemo_address()
            .unwrap();
        assert_eq!("15.15.15.1:10500".to_string(), addr_ip4.to_string());

        let addr_ip6 = Multiaddr(multiaddr!(
            Ip6([15, 15, 15, 15, 15, 15, 15, 1]),
            Udp(10500u16)
        ))
        .to_anemo_address()
        .unwrap();
        assert_eq!("[f:f:f:f:f:f:f:1]:10500".to_string(), addr_ip6.to_string());

        let addr_dns = Multiaddr(multiaddr!(Dns("iota.iota"), Udp(10501u16)))
            .to_anemo_address()
            .unwrap();
        assert_eq!("iota.iota:10501".to_string(), addr_dns.to_string());

        let addr_invalid =
            Multiaddr(multiaddr!(Dns("iota.iota"), Tcp(10501u16))).to_anemo_address();
        assert!(addr_invalid.is_err());
    }

    #[test]
    fn test_with_zero_ip() {
        let multi_addr_ip4 =
            Multiaddr(multiaddr!(Ip4([15, 15, 15, 1]), Tcp(10500u16))).with_zero_ip();
        assert_eq!(Some("0.0.0.0".to_string()), multi_addr_ip4.hostname());
        assert_eq!(Some(10500u16), multi_addr_ip4.port());

        let multi_addr_ip6 = Multiaddr(multiaddr!(
            Ip6([15, 15, 15, 15, 15, 15, 15, 1]),
            Tcp(10500u16)
        ))
        .with_zero_ip();
        assert_eq!(Some("::".to_string()), multi_addr_ip6.hostname());
        assert_eq!(Some(10500u16), multi_addr_ip4.port());

        let multi_addr_dns = Multiaddr(multiaddr!(Dns("iota.iota"), Tcp(10501u16))).with_zero_ip();
        assert_eq!(Some("0.0.0.0".to_string()), multi_addr_dns.hostname());
        assert_eq!(Some(10501u16), multi_addr_dns.port());
    }

    #[test]
    fn test_with_localhost_ip() {
        let multi_addr_ip4 =
            Multiaddr(multiaddr!(Ip4([15, 15, 15, 1]), Tcp(10500u16))).with_localhost_ip();
        assert_eq!(Some("127.0.0.1".to_string()), multi_addr_ip4.hostname());
        assert_eq!(Some(10500u16), multi_addr_ip4.port());

        let multi_addr_ip6 = Multiaddr(multiaddr!(
            Ip6([15, 15, 15, 15, 15, 15, 15, 1]),
            Tcp(10500u16)
        ))
        .with_localhost_ip();
        assert_eq!(Some("::1".to_string()), multi_addr_ip6.hostname());
        assert_eq!(Some(10500u16), multi_addr_ip4.port());

        let multi_addr_dns =
            Multiaddr(multiaddr!(Dns("iota.iota"), Tcp(10501u16))).with_localhost_ip();
        assert_eq!(Some("127.0.0.1".to_string()), multi_addr_dns.hostname());
        assert_eq!(Some(10501u16), multi_addr_dns.port());
    }

    #[test]
    fn test_is_private_or_unroutable_ipv4() {
        // Test cases: (multiaddr, description, should_be_filtered)
        let test_cases = vec![
            // Private addresses (RFC 1918)
            (
                multiaddr!(Ip4([10, 0, 0, 1]), Udp(10500u16)),
                "RFC 1918 private - 10.0.0.0/8",
                true,
            ),
            (
                multiaddr!(Ip4([172, 16, 0, 1]), Udp(10500u16)),
                "RFC 1918 private - 172.16.0.0/12",
                true,
            ),
            (
                multiaddr!(Ip4([192, 168, 1, 1]), Udp(10500u16)),
                "RFC 1918 private - 192.168.0.0/16",
                true,
            ),
            // Loopback (RFC 1122)
            (
                multiaddr!(Ip4([127, 0, 0, 1]), Udp(10500u16)),
                "RFC 1122 loopback",
                true,
            ),
            // Link-local (RFC 3927)
            (
                multiaddr!(Ip4([169, 254, 1, 1]), Udp(10500u16)),
                "RFC 3927 link-local",
                true,
            ),
            // Unspecified (RFC 1122)
            (
                multiaddr!(Ip4([0, 0, 0, 0]), Udp(10500u16)),
                "RFC 1122 unspecified",
                true,
            ),
            // Multicast (RFC 3171)
            (
                multiaddr!(Ip4([224, 0, 0, 1]), Udp(10500u16)),
                "RFC 3171 multicast",
                true,
            ),
            // Carrier-grade NAT (RFC 6598)
            (
                multiaddr!(Ip4([100, 64, 0, 1]), Udp(10500u16)),
                "RFC 6598 carrier-grade NAT",
                true,
            ),
            // IETF Protocol Assignments (RFC 6890)
            (
                multiaddr!(Ip4([192, 0, 0, 1]), Udp(10500u16)),
                "RFC 6890 IETF Protocol Assignments - 192.0.0.0/24",
                true,
            ),
            // AS112-v4 (RFC 7535)
            (
                multiaddr!(Ip4([192, 31, 196, 1]), Udp(10500u16)),
                "RFC 7535 AS112-v4 - 192.31.196.0/24",
                true,
            ),
            // Automatic Multicast Tunneling (RFC 7450)
            (
                multiaddr!(Ip4([192, 52, 193, 1]), Udp(10500u16)),
                "RFC 7450 Automatic Multicast Tunneling - 192.52.193.0/24",
                true,
            ),
            // Documentation addresses (RFC 5737)
            (
                multiaddr!(Ip4([192, 0, 2, 1]), Udp(10500u16)),
                "RFC 5737 documentation - 192.0.2.0/24",
                true,
            ),
            (
                multiaddr!(Ip4([198, 51, 100, 1]), Udp(10500u16)),
                "RFC 5737 documentation - 198.51.100.0/24",
                true,
            ),
            (
                multiaddr!(Ip4([203, 0, 113, 1]), Udp(10500u16)),
                "RFC 5737 documentation - 203.0.113.0/24",
                true,
            ),
            // Benchmarking (RFC 2544)
            (
                multiaddr!(Ip4([198, 18, 0, 1]), Udp(10500u16)),
                "RFC 2544 benchmarking - 198.18.0.0/15",
                true,
            ),
            // Public addresses should not be filtered
            (
                multiaddr!(Ip4([8, 8, 8, 8]), Udp(10500u16)),
                "Google DNS - should not be filtered",
                false,
            ),
            (
                multiaddr!(Ip4([1, 1, 1, 1]), Udp(10500u16)),
                "Cloudflare DNS - should not be filtered",
                false,
            ),
            (
                multiaddr!(Ip4([208, 67, 222, 222]), Udp(10500u16)),
                "OpenDNS - should not be filtered",
                false,
            ),
        ];

        for (multiaddr, description, should_be_filtered) in test_cases {
            let addr = Multiaddr(multiaddr);
            let is_filtered = addr.is_private_or_unroutable(false);
            assert_eq!(
                is_filtered, should_be_filtered,
                "Failed for {description}: expected {should_be_filtered} but got {is_filtered}",
            );
        }
    }

    #[test]
    fn test_is_private_or_unroutable_ipv6() {
        // Test cases: (multiaddr, description, should_be_filtered)
        let test_cases = vec![
            // Loopback (RFC 4291)
            (
                multiaddr!(Ip6([0, 0, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 4291 loopback",
                true,
            ),
            // Unspecified (RFC 4291)
            (
                multiaddr!(Ip6([0, 0, 0, 0, 0, 0, 0, 0]), Udp(10500u16)),
                "RFC 4291 unspecified",
                true,
            ),
            // Discard-Only Address Block (RFC 6666)
            (
                multiaddr!(Ip6([0x0100, 0, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 6666 discard-only - 100::/64",
                true,
            ),
            // Unique local addresses (RFC 4193) - fc00::/7
            (
                multiaddr!(Ip6([0xfc00, 0, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 4193 unique local - fc00::/7",
                true,
            ),
            (
                multiaddr!(Ip6([0xfd00, 0, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 4193 unique local - fd00::/7",
                true,
            ),
            // Link-local addresses (RFC 4862) - fe80::/10
            (
                multiaddr!(Ip6([0xfe80, 0, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 4862 link-local",
                true,
            ),
            // Benchmarking (RFC 5180) - 2001:2::/48
            (
                multiaddr!(Ip6([0x2001, 0x0002, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 5180 benchmarking - 2001:2::/48",
                true,
            ),
            // Documentation addresses (RFC 3849) - 2001:db8::/32
            (
                multiaddr!(Ip6([0x2001, 0x0db8, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "RFC 3849 documentation",
                true,
            ),
            // Multicast addresses
            (
                multiaddr!(Ip6([0xff02, 0, 0, 0, 0, 0, 0, 1]), Udp(10500u16)),
                "IPv6 multicast",
                true,
            ),
            // Public addresses should not be filtered
            (
                multiaddr!(
                    Ip6([0x2001, 0x4860, 0x4860, 0, 0, 0, 0, 0x68]),
                    Udp(10500u16)
                ),
                "Google DNS IPv6 - should not be filtered",
                false,
            ),
            (
                multiaddr!(Ip6([0x2606, 0x4700, 0x10, 0, 0, 0, 0, 0x68]), Udp(10500u16)),
                "Cloudflare DNS IPv6 - should not be filtered",
                false,
            ),
        ];

        for (multiaddr, description, should_be_filtered) in test_cases {
            let addr = Multiaddr(multiaddr);
            let is_filtered = addr.is_private_or_unroutable(false);
            assert_eq!(
                is_filtered, should_be_filtered,
                "Failed for {description}: expected {should_be_filtered} but got {is_filtered}",
            );
        }
    }

    #[test]
    fn test_is_private_or_unroutable_dns() {
        // Test cases: (multiaddr, description, should_be_private)
        let test_cases = vec![
            (
                multiaddr!(Dns("iota.org"), Udp(10500u16)),
                "DNS addresses should be allowed for public discovery",
                false,
            ),
            (
                multiaddr!(Dns4("iota.org"), Udp(10500u16)),
                "DNS4 addresses should be allowed for public discovery",
                false,
            ),
            (
                multiaddr!(Dns6("iota.org"), Udp(10500u16)),
                "DNS6 addresses should be allowed for public discovery",
                false,
            ),
        ];

        for (multiaddr, description, should_be_private) in test_cases {
            let addr = Multiaddr(multiaddr);
            let is_private = addr.is_private_or_unroutable(false);
            assert_eq!(
                is_private, should_be_private,
                "Failed for {description}: expected {should_be_private} but got {is_private}",
            );
        }
    }

    #[test]
    fn test_is_valid_for_public_announcement() {
        // Test cases: (multiaddr, description, should_be_valid)
        let test_cases = vec![
            (
                multiaddr!(Ip4([192, 168, 1, 1]), Udp(10500u16)),
                "Private IPv4 address should be invalid",
                false,
            ),
            (
                multiaddr!(Ip4([127, 0, 0, 1]), Udp(10500u16)),
                "Loopback IPv4 address should be invalid",
                false,
            ),
            (
                multiaddr!(Ip4([8, 8, 8, 8]), Udp(10500u16)),
                "Valid public IPv4 address should be valid",
                true,
            ),
            (
                multiaddr!(Dns("example.com"), Udp(10500u16)),
                "Valid DNS address should be valid",
                true,
            ),
            (
                multiaddr!(Ip4([8, 8, 8, 8]), Tcp(10500u16)),
                "TCP instead of UDP should be invalid for anemo",
                false,
            ),
        ];

        for (multiaddr, description, should_be_valid) in test_cases {
            let addr = Multiaddr(multiaddr);
            let is_valid = addr.is_valid_public_anemo_address(false);
            assert_eq!(
                is_valid, should_be_valid,
                "Failed for {description}: expected {should_be_valid} but got {is_valid}",
            );
        }
    }

    #[test]
    fn test_is_valid_fqdn() {
        use super::is_valid_fqdn;

        // Valid FQDNs - domains with proper structure
        let valid_cases = vec![
            "example.com",
            "subdomain.example.com",
            "iota.org",
            "api.iota.org",
            "test123.example-domain.org",
            "google.com",
            "github.com",
            "example.co", // Two-letter TLD
            "test.example.net",
            "api-v1.service.io",
            "very-long-subdomain-name-that-is-still-valid.example.org",
            // Unicode domains (IDNA)
            "café.com",         // Unicode characters
            "москва.рф",        // Cyrillic
            "τεστ.gr",          // Greek
            "测试.中国",        // Chinese
            "xn--nxasmq6b.com", // Already punycode-encoded domain
        ];

        for fqdn in valid_cases {
            assert!(
                is_valid_fqdn(fqdn, false),
                "Expected '{fqdn}' to be a valid FQDN",
            );
        }

        // Invalid FQDNs
        let long_domain = "a".repeat(254);
        let long_label = "a".repeat(64);
        let long_label_domain = format!("{long_label}.com");
        let invalid_cases = vec![
            "",                 // Empty string
            "localhost",        // Localhost should be rejected
            "test.local",       // .local domain
            "hostname",         // Single label (no TLD)
            "a.b",              // Single char TLD
            "example.1",        // Numeric TLD
            "example.c1",       // Alphanumeric TLD
            ".",                // Just a dot
            ".example.com",     // Leading dot
            "example.com.",     // Trailing dot (absolute FQDN)
            "-example.com",     // Label starting with hyphen
            "example-.com",     // Label ending with hyphen
            "exam_ple.com",     // Underscore not allowed
            "exam ple.com",     // Space not allowed
            &long_domain,       // Domain name too long (254 chars)
            &long_label_domain, // Label too long (64 chars)
            "example..com",     // Double dot
            "192.168.1.1",      // IP address (should use IP protocols)
            "2001:db8::1",      // IPv6 address (should use IP protocols)
            "example.com-",     // Trailing hyphen in TLD
            "example.",         // Empty TLD
            // Invalid Unicode domains
            "invalid\u{200D}.com", // Zero-width joiner (invalid in domain)
            "test\u{0000}.com",    // Null character (invalid)
        ];

        for fqdn in invalid_cases {
            assert!(
                !is_valid_fqdn(fqdn, false),
                "Expected '{fqdn}' to be an invalid FQDN"
            );
        }
    }

    #[test]
    fn test_is_valid_dns_label() {
        use super::is_valid_dns_label;

        // Valid DNS labels
        let max_length_label = "a".repeat(63);
        let valid_cases = vec![
            "a",
            "ab",
            "example",
            "test123",
            "api-v1",
            "sub-domain",
            "a1b2c3",
            "123",             // All numeric is valid per RFC 1035
            &max_length_label, // Max length (63 chars)
        ];

        for label in valid_cases {
            assert!(
                is_valid_dns_label(label),
                "Expected '{label}' to be a valid DNS label"
            );
        }

        // Invalid DNS labels
        let too_long_label = "a".repeat(64);
        let invalid_cases = vec![
            "",              // Empty
            "-example",      // Starts with hyphen
            "example-",      // Ends with hyphen
            "ex_ample",      // Contains underscore
            "ex ample",      // Contains space
            "ex.ample",      // Contains dot
            &too_long_label, // Too long (64 chars)
        ];

        for label in invalid_cases {
            assert!(
                !is_valid_dns_label(label),
                "Expected '{label}' to be an invalid DNS label"
            );
        }
    }

    #[test]
    fn test_dns_validation_in_multiaddr() {
        // Test that DNS validation is properly integrated into multiaddr validation

        // Valid DNS addresses should not be filtered
        let valid_dns_cases = vec![
            multiaddr!(Dns("example.com"), Udp(10500u16)),
            multiaddr!(Dns("iota.org"), Udp(10500u16)),
            multiaddr!(Dns("api.example.com"), Udp(10500u16)),
            // Unicode domains
            multiaddr!(Dns("café.com"), Udp(10500u16)),
            multiaddr!(Dns("москва.рф"), Udp(10500u16)),
        ];

        for addr in valid_dns_cases {
            let multiaddr = Multiaddr(addr);
            assert!(
                !multiaddr.is_private_or_unroutable(false),
                "Valid DNS address {multiaddr} should not be filtered as private/unroutable"
            );
            assert!(
                multiaddr.is_valid_public_anemo_address(false),
                "Valid DNS address {multiaddr} should be valid for public announcement"
            );
        }

        // Invalid DNS addresses should be filtered
        let invalid_dns_cases = vec![
            multiaddr!(Dns("localhost"), Udp(10500u16)),
            multiaddr!(Dns("hostname.local"), Udp(10500u16)),
            multiaddr!(Dns("hostname"), Udp(10500u16)), // Single label
            multiaddr!(Dns(""), Udp(10500u16)),         // Empty
        ];

        for addr in invalid_dns_cases {
            let multiaddr = Multiaddr(addr);
            assert!(
                multiaddr.is_private_or_unroutable(false),
                "Invalid DNS address {multiaddr} should be filtered as private/unroutable"
            );
            assert!(
                !multiaddr.is_valid_public_anemo_address(false),
                "Invalid DNS address {multiaddr} should not be valid for public announcement"
            );
        }
    }
}
