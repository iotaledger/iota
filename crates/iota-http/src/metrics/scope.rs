// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Where a client address is: on this host, in a private network, or on the
/// internet.
#[derive(Clone, Copy, Debug, PartialEq, Eq, strum::IntoStaticStr)]
#[strum(serialize_all = "snake_case")]
pub(super) enum Scope {
    Loopback,
    Private,
    Public,
}

impl Scope {
    /// The scope of `ip`. An IPv4-mapped IPv6 address has the scope of the IPv4
    /// address.
    pub(super) fn of(ip: IpAddr) -> Self {
        match ip.to_canonical() {
            IpAddr::V4(ip) => Self::of_ipv4(ip),
            IpAddr::V6(ip) => Self::of_ipv6(ip),
        }
    }

    fn of_ipv4(ip: Ipv4Addr) -> Self {
        // RFC 6598 shared address space, 100.64.0.0/10: `Ipv4Addr::is_shared` is
        // unstable.
        let shared = ip.octets()[0] == 100 && ip.octets()[1] & 0xc0 == 64;
        if ip.is_loopback() {
            Self::Loopback
        } else if ip.is_private() || ip.is_link_local() || shared {
            Self::Private
        } else {
            Self::Public
        }
    }

    fn of_ipv6(ip: Ipv6Addr) -> Self {
        if ip.is_loopback() {
            Self::Loopback
        } else if ip.is_unique_local() || ip.is_unicast_link_local() {
            Self::Private
        } else {
            Self::Public
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ip(s: &str) -> IpAddr {
        s.parse().unwrap()
    }

    #[test]
    fn mapped_ipv4_has_the_scope_of_the_ipv4_address() {
        assert_eq!(Scope::of(ip("::ffff:127.0.0.1")), Scope::Loopback);
        assert_eq!(Scope::of(ip("::ffff:10.1.2.3")), Scope::Private);
    }

    #[test]
    fn scopes() {
        for (address, scope) in [
            ("127.0.0.1", Scope::Loopback),
            ("127.9.9.9", Scope::Loopback),
            ("::1", Scope::Loopback),
            ("10.0.0.1", Scope::Private),
            ("172.16.0.1", Scope::Private),
            ("172.31.255.255", Scope::Private),
            ("172.32.0.1", Scope::Public),
            ("192.168.1.1", Scope::Private),
            ("169.254.1.1", Scope::Private),
            ("100.64.0.1", Scope::Private),
            ("100.127.255.255", Scope::Private),
            ("100.128.0.1", Scope::Public),
            ("fd00::1", Scope::Private),
            ("fe80::1", Scope::Private),
            ("1.2.3.4", Scope::Public),
            ("2001:db8::1", Scope::Public),
        ] {
            assert_eq!(Scope::of(ip(address)), scope, "{address}");
        }
    }
}
