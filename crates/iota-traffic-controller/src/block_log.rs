// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! The line the node logs when it starts to refuse a client. Host firewall
//! tools such as fail2ban and CrowdSec match on this line, so its format is
//! part of the node's interface to operators: change it only together with
//! the filters and operator docs that match on it, and mention the change in
//! the release notes.

use std::{fmt, net::IpAddr};

use tracing::info;

/// Tracing target of the block log line.
pub(crate) const BLOCK_LOG_TARGET: &str = "iota_node::client_blocked";

/// The policy whose limit the client exceeded.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReason {
    Spam,
    Error,
}

impl fmt::Display for BlockReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Spam => "spam",
            Self::Error => "error",
        })
    }
}

/// Logs that the node refuses `client` for the next `ttl_secs` seconds. Call
/// it once when a block takes effect, not for every refused request, and not
/// for a block that dry run keeps from taking effect.
///
/// In the plain-text log the line carries no span, so that it reads the same
/// whichever request caused the block. The JSON log may still add the span of
/// that request after the `target` key.
pub(crate) fn log_block(client: IpAddr, reason: BlockReason, ttl_secs: u64) {
    info!(
        target: BLOCK_LOG_TARGET,
        parent: None,
        "{}",
        block_log_line(client, reason, ttl_secs)
    );
}

/// An IPv4 client that reached a dual-stack socket shows up as an
/// IPv4-mapped IPv6 address. The line names it by its IPv4 address, which is
/// the one a host firewall sees.
fn block_log_line(client: IpAddr, reason: BlockReason, ttl_secs: u64) -> String {
    let client = client.to_canonical();
    format!("client_blocked client={client} reason={reason} ttl_secs={ttl_secs}")
}

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        io,
        net::{Ipv4Addr, Ipv6Addr},
        sync::{Arc, Mutex},
    };

    use tracing_subscriber::fmt::MakeWriter;

    use super::*;

    /// Collects what a subscriber writes, for tests to read back.
    #[derive(Clone, Default)]
    pub(crate) struct CapturedLog(Arc<Mutex<Vec<u8>>>);

    impl CapturedLog {
        pub(crate) fn lines(&self) -> Vec<String> {
            String::from_utf8(self.0.lock().unwrap().clone())
                .unwrap()
                .lines()
                .map(str::to_owned)
                .collect()
        }

        /// The block log lines written so far.
        pub(crate) fn block_lines(&self) -> Vec<String> {
            self.lines()
                .into_iter()
                .filter(|line| line.contains("client_blocked "))
                .collect()
        }
    }

    impl io::Write for CapturedLog {
        fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
            self.0.lock().unwrap().extend_from_slice(buf);
            Ok(buf.len())
        }

        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }

    impl<'a> MakeWriter<'a> for CapturedLog {
        type Writer = Self;

        fn make_writer(&'a self) -> Self::Writer {
            self.clone()
        }
    }

    /// Runs `f` with a subscriber that writes plain text as the node does
    /// without `RUST_LOG_JSON`, and returns what it wrote.
    pub(crate) fn capture_plain(f: impl FnOnce()) -> CapturedLog {
        let log = CapturedLog::default();
        let subscriber = tracing_subscriber::fmt()
            .with_ansi(false)
            .with_writer(log.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        log
    }

    /// Runs `f` with a subscriber that writes JSON as the node does with
    /// `RUST_LOG_JSON`, and returns what it wrote.
    fn capture_json(f: impl FnOnce()) -> CapturedLog {
        let log = CapturedLog::default();
        let subscriber = tracing_subscriber::fmt()
            .with_file(true)
            .with_line_number(true)
            .json()
            .with_writer(log.clone())
            .finish();
        tracing::subscriber::with_default(subscriber, f);
        log
    }

    #[test]
    fn the_line_names_the_client_the_reason_and_the_ttl() {
        let v4 = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));
        assert_eq!(
            block_log_line(v4, BlockReason::Spam, 300),
            "client_blocked client=203.0.113.7 reason=spam ttl_secs=300"
        );
        let v6 = IpAddr::V6(Ipv6Addr::new(0x2001, 0xdb8, 1, 2, 0, 0, 0, 1));
        assert_eq!(
            block_log_line(v6, BlockReason::Error, 300),
            "client_blocked client=2001:db8:1:2::1 reason=error ttl_secs=300"
        );
    }

    #[test]
    fn an_ipv4_mapped_client_is_named_by_its_ipv4_address() {
        let mapped = IpAddr::V6(Ipv4Addr::new(203, 0, 113, 7).to_ipv6_mapped());
        assert_eq!(
            block_log_line(mapped, BlockReason::Spam, 60),
            "client_blocked client=203.0.113.7 reason=spam ttl_secs=60"
        );
    }

    #[test]
    fn the_plain_text_output_holds_the_line_under_its_target() {
        let client = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));
        let log = capture_plain(|| log_block(client, BlockReason::Spam, 300));

        let [line] = log.lines().try_into().unwrap();
        assert!(
            line.ends_with(
                " INFO iota_node::client_blocked: \
                 client_blocked client=203.0.113.7 reason=spam ttl_secs=300"
            ),
            "{line}"
        );
    }

    #[test]
    fn the_line_carries_no_span_of_the_request() {
        let client = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));
        let log = capture_plain(|| {
            tracing::info_span!("request", peer = "203.0.113.7")
                .in_scope(|| log_block(client, BlockReason::Spam, 300))
        });

        let [line] = log.lines().try_into().unwrap();
        assert!(
            line.ends_with(
                " INFO iota_node::client_blocked: \
                 client_blocked client=203.0.113.7 reason=spam ttl_secs=300"
            ),
            "{line}"
        );
    }

    #[test]
    fn the_json_output_holds_the_line_as_its_message() {
        let client = IpAddr::V4(Ipv4Addr::new(203, 0, 113, 7));
        let log = capture_json(|| log_block(client, BlockReason::Error, 300));

        let [line] = log.lines().try_into().unwrap();
        let event: serde_json::Value = serde_json::from_str(&line).unwrap();
        assert_eq!(event["level"], "INFO");
        assert_eq!(event["target"], BLOCK_LOG_TARGET);
        assert_eq!(
            event["fields"]["message"],
            "client_blocked client=203.0.113.7 reason=error ttl_secs=300"
        );
    }
}
