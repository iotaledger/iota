// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Minimal wasm-compatible stand-in for `iota_network_stack::multiaddr`.
//!
//! On wasm32 the real `iota-network-stack` crate doesn't compile (it pulls in
//! the full networking stack — tonic, anemo, axum). For execution we don't
//! need any of the network functionality on `Multiaddr`; we only need:
//!
//! - struct fields on validator metadata types to type-check
//! - BCS / serde round-trip compatible with the wire format
//!
//! The on-chain serialization is just a UTF-8 string (see
//! `iota-network-stack/src/multiaddr.rs`), so this stub matches that.

use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Hash, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Multiaddr(String);

impl Multiaddr {
    pub fn empty() -> Self {
        Self(String::new())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Multiaddr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

// Only `TryFrom<String>` — matches the real `Multiaddr` API used by callers
// in `iota_system_state_inner_v1`. A blanket `From<String>` would conflict
// with the std blanket `TryFrom<T> for T where T: From<U>` impl.
impl TryFrom<String> for Multiaddr {
    type Error = std::convert::Infallible;
    fn try_from(s: String) -> Result<Self, Self::Error> {
        Ok(Multiaddr(s))
    }
}

impl Serialize for Multiaddr {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.0)
    }
}

impl<'de> Deserialize<'de> for Multiaddr {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        Ok(Multiaddr(s))
    }
}
