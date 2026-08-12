// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Minimal wasm-compatible stand-in for the native [`Multiaddr`].
//!
//! The native implementation doesn't compile for wasm32: it needs `anemo` for
//! `to_anemo_address`, and the address parsing helpers are only useful to a
//! node. For execution we don't need any of that on `Multiaddr`; we only need:
//!
//! - struct fields on validator metadata types to type-check
//! - BCS / serde round-trip compatible with the wire format
//!
//! The on-chain serialization is just a UTF-8 string (see `native.rs`), so this
//! stand-in matches that. It does not validate the string on deserialization,
//! where the native implementation parses it; on wasm we are inspecting
//! already-on-chain state, which the Move verifier gated at validator creation.
//!
//! [`Multiaddr`]: super::Multiaddr

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
