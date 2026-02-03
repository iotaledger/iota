// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{env, fmt};

use fastcrypto::encoding::{Base58, Encoding, Hex};
use iota_protocol_config::Chain;
pub use iota_sdk_types::Digest;
use once_cell::sync::{Lazy, OnceCell};
use serde::{Deserialize, Serialize};
use tracing::info;

/// Representation of a network's identifier by the genesis checkpoint's digest
#[derive(
    Clone, Copy, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
pub struct ChainIdentifier(pub(crate) CheckpointDigest);

pub const MAINNET_CHAIN_IDENTIFIER_BASE58: &str = "7gzPnGnmjqvpmF7NXTCmtacXLqx1cMaJFV6GCmi1peqr";
pub const TESTNET_CHAIN_IDENTIFIER_BASE58: &str = "3MhPzSaSTHGffwPSV2Ws2DaK8LR8DBGPozTd2CbiJRwe";

pub static MAINNET_CHAIN_IDENTIFIER: OnceCell<ChainIdentifier> = OnceCell::new();
pub static TESTNET_CHAIN_IDENTIFIER: OnceCell<ChainIdentifier> = OnceCell::new();

/// For testing purposes or bootstrapping regenesis chain configuration, you can
/// set this environment variable to force protocol config to use a specific
/// Chain.
const IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE_ENV_VAR_NAME: &str =
    "IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE";

static IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE: Lazy<Option<Chain>> = Lazy::new(|| {
    if let Ok(s) = env::var(IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE_ENV_VAR_NAME) {
        info!("IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE: {:?}", s);
        match s.as_str() {
            "mainnet" => Some(Chain::Mainnet),
            "testnet" => Some(Chain::Testnet),
            "" => None,
            _ => panic!("unrecognized IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE: {s:?}"),
        }
    } else {
        None
    }
});

impl ChainIdentifier {
    /// Take a short 4 byte identifier and convert it into a ChainIdentifier.
    /// Short ids come from the JSON RPC getChainIdentifier and are encoded in
    /// hex.
    pub fn from_chain_short_id(short_id: impl AsRef<str>) -> Option<Self> {
        if Hex::from_bytes(&Base58::decode(MAINNET_CHAIN_IDENTIFIER_BASE58).ok()?)
            .encoded_with_format()
            .starts_with(&format!("0x{}", short_id.as_ref()))
        {
            Some(get_mainnet_chain_identifier())
        } else if Hex::from_bytes(&Base58::decode(TESTNET_CHAIN_IDENTIFIER_BASE58).ok()?)
            .encoded_with_format()
            .starts_with(&format!("0x{}", short_id.as_ref()))
        {
            Some(get_testnet_chain_identifier())
        } else {
            None
        }
    }

    pub fn chain(&self) -> Chain {
        let mainnet_id = get_mainnet_chain_identifier();
        let testnet_id = get_testnet_chain_identifier();

        let chain = match self {
            id if *id == mainnet_id => Chain::Mainnet,
            id if *id == testnet_id => Chain::Testnet,
            _ => Chain::Unknown,
        };
        if let Some(override_chain) = *IOTA_PROTOCOL_CONFIG_CHAIN_OVERRIDE {
            if chain != Chain::Unknown {
                panic!("not allowed to override real chain {chain:?}");
            }
            return override_chain;
        }

        chain
    }

    pub fn as_bytes(&self) -> &[u8; 32] {
        self.0.inner()
    }

    pub fn into_bytes(self) -> [u8; 32] {
        self.0.into_inner()
    }

    pub fn digest(&self) -> CheckpointDigest {
        self.0
    }
}

pub fn get_mainnet_chain_identifier() -> ChainIdentifier {
    let digest = MAINNET_CHAIN_IDENTIFIER.get_or_init(|| {
        let digest = CheckpointDigest::new(
            Base58::decode(MAINNET_CHAIN_IDENTIFIER_BASE58)
                .expect("mainnet genesis checkpoint digest literal is invalid")
                .try_into()
                .expect("Mainnet genesis checkpoint digest literal has incorrect length"),
        );
        ChainIdentifier::from(digest)
    });
    *digest
}

pub fn get_testnet_chain_identifier() -> ChainIdentifier {
    let digest = TESTNET_CHAIN_IDENTIFIER.get_or_init(|| {
        let digest = CheckpointDigest::new(
            Base58::decode(TESTNET_CHAIN_IDENTIFIER_BASE58)
                .expect("testnet genesis checkpoint digest literal is invalid")
                .try_into()
                .expect("Testnet genesis checkpoint digest literal has incorrect length"),
        );
        ChainIdentifier::from(digest)
    });
    *digest
}

impl fmt::Display for ChainIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0.as_bytes()[0..4].iter() {
            write!(f, "{byte:02x}")?;
        }

        Ok(())
    }
}

impl From<CheckpointDigest> for ChainIdentifier {
    fn from(digest: CheckpointDigest) -> Self {
        Self(digest)
    }
}

pub type CheckpointDigest = Digest;
pub type CheckpointContentsDigest = Digest;
pub type CertificateDigest = Digest;
pub type SenderSignedDataDigest = Digest;
pub type TransactionDigest = Digest;
pub type TransactionEffectsDigest = Digest;
pub type TransactionEventsDigest = Digest;
pub type EffectsAuxDataDigest = Digest;
pub type ObjectDigest = Digest;
pub type ConsensusCommitDigest = Digest;
pub type MoveAuthenticatorDigest = Digest;
pub type MisbehaviorReportDigest = Digest;

mod test {
    #[allow(unused_imports)]
    use crate::digests::ChainIdentifier;

    #[test]
    fn test_chain_id_mainnet() {
        let chain_id = ChainIdentifier::from_chain_short_id("6364aad5");
        assert_eq!(
            chain_id.unwrap().chain(),
            iota_protocol_config::Chain::Mainnet
        );
    }

    #[test]
    fn test_chain_id_testnet() {
        let chain_id = ChainIdentifier::from_chain_short_id("2304aa97");
        assert_eq!(
            chain_id.unwrap().chain(),
            iota_protocol_config::Chain::Testnet
        );
    }

    #[test]
    fn test_chain_id_unknown() {
        let chain_id = ChainIdentifier::from_chain_short_id("unknown");
        assert_eq!(chain_id, None);
    }
}
