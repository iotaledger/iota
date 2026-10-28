// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{
    net::{IpAddr, Ipv6Addr},
    time::Duration,
};

/// Types that can listen for connections.
pub trait Listener: Send + 'static {
    /// The listener's IO type.
    type Io: tokio::io::AsyncRead + tokio::io::AsyncWrite + Unpin + Send + 'static;

    /// The listener's address type.
    // all these bounds are necessary to add this information in a request extension
    type Addr: Clone + Send + Sync + 'static;

    /// Accept a new incoming connection to this listener.
    ///
    /// If the underlying accept call can return an error, this function must
    /// take care of logging and retrying.
    fn accept(&mut self) -> impl std::future::Future<Output = (Self::Io, Self::Addr)> + Send;

    /// Returns the local address that this listener is bound to.
    fn local_addr(&self) -> std::io::Result<Self::Addr>;

    /// Identifies the client at `client`, the remote address of a connection
    /// this listener accepted. Connections with equal keys count as one client
    /// under `max_connections_per_client_ip`.
    fn client_prefix(&self, client: &Self::Addr) -> Vec<u8>;
}

/// Extensions to [`Listener`].
pub trait ListenerExt: Listener + Sized {
    /// Run a mutable closure on every accepted `Io`.
    ///
    /// # Example
    ///
    /// ```
    /// use iota_http::ListenerExt;
    /// use tracing::trace;
    ///
    /// # async {
    /// let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
    ///     .await
    ///     .unwrap()
    ///     .tap_io(|tcp_stream| {
    ///         if let Err(err) = tcp_stream.set_nodelay(true) {
    ///             trace!("failed to set TCP_NODELAY on incoming connection: {err:#}");
    ///         }
    ///     });
    /// # };
    /// ```
    fn tap_io<F>(self, tap_fn: F) -> TapIo<Self, F>
    where
        F: FnMut(&mut Self::Io) + Send + 'static,
    {
        TapIo {
            listener: self,
            tap_fn,
        }
    }
}

impl<L: Listener> ListenerExt for L {}

impl Listener for tokio::net::TcpListener {
    type Io = tokio::net::TcpStream;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        loop {
            match Self::accept(self).await {
                Ok(tup) => return tup,
                Err(e) => handle_accept_error(e).await,
            }
        }
    }

    #[inline]
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Self::local_addr(self)
    }

    fn client_prefix(&self, client: &Self::Addr) -> Vec<u8> {
        // A bare listener has no settings to take a prefix length from.
        client_prefix(client.ip(), DEFAULT_IPV6_PREFIX_LEN)
    }
}

#[derive(Debug)]
pub struct TcpListenerWithOptions {
    inner: tokio::net::TcpListener,
    keepalive: Option<Duration>,
    ipv6_prefix_len: u8,
}

impl TcpListenerWithOptions {
    pub fn new<A: std::net::ToSocketAddrs>(
        addr: A,
        keepalive: Option<Duration>,
        ipv6_prefix_len: u8,
    ) -> Result<Self, crate::BoxError> {
        let std_listener = std::net::TcpListener::bind(addr)?;
        std_listener.set_nonblocking(true)?;
        let listener = tokio::net::TcpListener::from_std(std_listener)?;

        Ok(Self::from_listener(listener, keepalive, ipv6_prefix_len))
    }

    /// Creates a new `TcpIncoming` from an existing `tokio::net::TcpListener`.
    pub fn from_listener(
        listener: tokio::net::TcpListener,
        keepalive: Option<Duration>,
        ipv6_prefix_len: u8,
    ) -> Self {
        Self {
            inner: listener,
            keepalive,
            ipv6_prefix_len,
        }
    }

    // Consistent with hyper-0.14, this function does not return an error.
    fn set_accepted_socket_options(&self, stream: &tokio::net::TcpStream) {
        if let Err(e) = stream.set_nodelay(true) {
            tracing::warn!("error trying to set TCP nodelay: {}", e);
        }

        if let Some(timeout) = self.keepalive {
            let sock_ref = socket2::SockRef::from(&stream);
            let sock_keepalive = socket2::TcpKeepalive::new().with_time(timeout);

            if let Err(e) = sock_ref.set_tcp_keepalive(&sock_keepalive) {
                tracing::warn!("error trying to set TCP keepalive: {}", e);
            }
        }
    }
}

impl Listener for TcpListenerWithOptions {
    type Io = tokio::net::TcpStream;
    type Addr = std::net::SocketAddr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (io, addr) = Listener::accept(&mut self.inner).await;
        self.set_accepted_socket_options(&io);
        (io, addr)
    }

    #[inline]
    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        Listener::local_addr(&self.inner)
    }

    fn client_prefix(&self, client: &Self::Addr) -> Vec<u8> {
        client_prefix(client.ip(), self.ipv6_prefix_len)
    }
}

// Uncomment once we update tokio to >=1.41.0
// #[cfg(unix)]
// impl Listener for tokio::net::UnixListener {
//     type Io = tokio::net::UnixStream;
//     type Addr = std::os::unix::net::SocketAddr;

//     async fn accept(&mut self) -> (Self::Io, Self::Addr) {
//         loop {
//             match Self::accept(self).await {
//                 Ok((io, addr)) => return (io, addr.into()),
//                 Err(e) => handle_accept_error(e).await,
//             }
//         }
//     }

//     #[inline]
//     fn local_addr(&self) -> std::io::Result<Self::Addr> {
//         Self::local_addr(self).map(Into::into)
//     }
// }

/// Return type of [`ListenerExt::tap_io`].
///
/// See that method for details.
pub struct TapIo<L, F> {
    listener: L,
    tap_fn: F,
}

impl<L, F> std::fmt::Debug for TapIo<L, F>
where
    L: Listener + std::fmt::Debug,
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("TapIo")
            .field("listener", &self.listener)
            .finish_non_exhaustive()
    }
}

impl<L, F> Listener for TapIo<L, F>
where
    L: Listener,
    F: FnMut(&mut L::Io) + Send + 'static,
{
    type Io = L::Io;
    type Addr = L::Addr;

    async fn accept(&mut self) -> (Self::Io, Self::Addr) {
        let (mut io, addr) = self.listener.accept().await;
        (self.tap_fn)(&mut io);
        (io, addr)
    }

    fn local_addr(&self) -> std::io::Result<Self::Addr> {
        self.listener.local_addr()
    }

    fn client_prefix(&self, client: &Self::Addr) -> Vec<u8> {
        self.listener.client_prefix(client)
    }
}

async fn handle_accept_error(e: std::io::Error) {
    if is_connection_error(&e) {
        return;
    }

    // [From `hyper::Server` in 0.14](https://github.com/hyperium/hyper/blob/v0.14.27/src/server/tcp.rs#L186)
    //
    // > A possible scenario is that the process has hit the max open files
    // > allowed, and so trying to accept a new connection will fail with
    // > `EMFILE`. In some cases, it's preferable to just wait for some time, if
    // > the application will likely close some files (or connections), and try
    // > to accept the connection again. If this option is `true`, the error
    // > will be logged at the `error` level, since it is still a big deal,
    // > and then the listener will sleep for 1 second.
    //
    // hyper allowed customizing this but axum does not.
    tracing::error!("accept error: {e}");
    tokio::time::sleep(Duration::from_secs(1)).await;
}

fn is_connection_error(e: &std::io::Error) -> bool {
    use std::io::ErrorKind;

    matches!(
        e.kind(),
        ErrorKind::ConnectionRefused
            | ErrorKind::ConnectionAborted
            | ErrorKind::ConnectionReset
            | ErrorKind::BrokenPipe
            | ErrorKind::Interrupted
            | ErrorKind::WouldBlock
            | ErrorKind::TimedOut
    )
}

/// How many leading bits of an IPv6 address identify its client when no other
/// length is configured: the size of the network a single site is usually
/// given.
pub(crate) const DEFAULT_IPV6_PREFIX_LEN: u8 = 64;

/// The shortest IPv6 prefix length accepted. The ranges that
/// [`ipv6_prefix_len_of`] keeps whole are set by at most the first 32 bits, so
/// cutting any other address to 32 bits or more never lands it in one.
pub(crate) const MIN_IPV6_PREFIX_LEN: u8 = 32;

/// The key of the client at `client`. An IPv4 client is its full address. An
/// IPv6 client is the first `ipv6_prefix_len` bits of its address, with the
/// rest set to zero: the server cannot see how long a client's own network
/// prefix is, so it assumes the same length for every IPv6 client. An IPv4
/// client seen as an IPv4-mapped IPv6 address counts as that IPv4 address.
/// See [`ipv6_prefix_len_of`] for the IPv6 addresses kept whole.
///
/// The key of an IPv4 client is 4 bytes long and that of an IPv6 client 16,
/// so the two never collide.
fn client_prefix(client: IpAddr, ipv6_prefix_len: u8) -> Vec<u8> {
    match client.to_canonical() {
        IpAddr::V6(address) => {
            let prefix_len = ipv6_prefix_len_of(address, ipv6_prefix_len);
            let mask = u128::MAX
                .checked_shl(128 - u32::from(prefix_len.min(128)))
                .unwrap_or(0);
            (u128::from(address) & mask).to_be_bytes().to_vec()
        }
        IpAddr::V4(address) => address.octets().to_vec(),
    }
}

/// How many leading bits of `address` identify its client: `ipv6_prefix_len`,
/// or all 128 where unrelated clients share the leading bits. These are
/// `::/32`, with the unspecified, loopback and IPv4-compatible addresses,
/// link-local addresses (`fe80::/10`), Teredo addresses (`2001::/32`), and
/// the NAT64 prefixes in `64:ff9b::/32`, each of whose addresses carries one
/// IPv4 client.
fn ipv6_prefix_len_of(address: Ipv6Addr, ipv6_prefix_len: u8) -> u8 {
    let segments = address.segments();
    let kept_whole = segments[..2] == [0, 0]
        || segments[0] & 0xffc0 == 0xfe80
        || segments[..2] == [0x2001, 0]
        || segments[..2] == [0x64, 0xff9b];
    if kept_whole { 128 } else { ipv6_prefix_len }
}

#[cfg(test)]
mod tests {
    use std::net::Ipv4Addr;

    use super::*;

    const CLIENT: IpAddr = IpAddr::V4(Ipv4Addr::new(10, 0, 0, 1));

    fn v6(segments: [u16; 8]) -> IpAddr {
        IpAddr::V6(Ipv6Addr::from(segments))
    }

    fn key(segments: [u16; 8]) -> Vec<u8> {
        Ipv6Addr::from(segments).octets().to_vec()
    }

    #[test]
    fn an_ipv6_client_is_its_address_prefix() {
        let client = v6([0x2001, 0xdb8, 1, 2, 3, 4, 5, 6]);
        assert_eq!(
            client_prefix(client, 64),
            key([0x2001, 0xdb8, 1, 2, 0, 0, 0, 0])
        );
        assert_eq!(
            client_prefix(client, 48),
            key([0x2001, 0xdb8, 1, 0, 0, 0, 0, 0])
        );
        assert_eq!(
            client_prefix(client, 128),
            key([0x2001, 0xdb8, 1, 2, 3, 4, 5, 6])
        );
    }

    #[test]
    fn an_ipv4_client_is_its_full_address() {
        assert_eq!(client_prefix(CLIENT, 64), vec![10, 0, 0, 1]);
        let IpAddr::V4(address) = CLIENT else {
            unreachable!()
        };
        let mapped = IpAddr::V6(address.to_ipv6_mapped());
        assert_eq!(client_prefix(mapped, 64), vec![10, 0, 0, 1]);
    }

    #[test]
    fn an_ipv4_client_never_shares_a_key_with_an_ipv6_client() {
        // The IPv4-compatible form of the same address is kept whole, so only
        // the key's length tells the two apart.
        let compatible = v6([0, 0, 0, 0, 0, 0, 0x0a00, 0x0001]);
        assert_ne!(client_prefix(CLIENT, 64), client_prefix(compatible, 64));
    }

    #[test]
    fn ipv6_addresses_whose_leading_bits_are_shared_are_kept_whole() {
        for client in [
            v6([0, 0, 0, 0, 0, 0, 0, 1]),
            v6([0, 0, 0, 0, 0, 0, 0x0a00, 0x0001]),
            v6([0xfe80, 0, 0, 0, 1, 2, 3, 4]),
            v6([0x2001, 0, 0x4136, 0xe378, 0x8000, 0x63bf, 0x3fff, 0xfdd2]),
            v6([0x64, 0xff9b, 0, 0, 0, 0, 0x0a00, 0x0001]),
            v6([0x64, 0xff9b, 1, 0x0a00, 0x0001, 0, 0, 0]),
        ] {
            let IpAddr::V6(address) = client else {
                unreachable!()
            };
            assert_eq!(client_prefix(client, 64), address.octets().to_vec());
        }
    }

    #[tokio::test]
    async fn a_tcp_listener_cuts_ipv6_clients_to_its_prefix_length() {
        let listener = TcpListenerWithOptions::new("127.0.0.1:0", None, 48).unwrap();
        let client = |segments| std::net::SocketAddr::new(v6(segments), 1);

        assert_eq!(
            listener.client_prefix(&client([0x2001, 0xdb8, 1, 2, 0, 0, 0, 1])),
            listener.client_prefix(&client([0x2001, 0xdb8, 1, 3, 0, 0, 0, 1])),
            "addresses in one /48 must be one client"
        );
        assert_ne!(
            listener.client_prefix(&client([0x2001, 0xdb8, 1, 0, 0, 0, 0, 1])),
            listener.client_prefix(&client([0x2001, 0xdb8, 2, 0, 0, 0, 0, 1])),
            "addresses in different /48s must be different clients"
        );
    }
}
