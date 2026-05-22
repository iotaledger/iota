// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
//
// On-disk cache for the bench-driver gas-coin pool. The full setup
// (`bank.generate(...)` → `pay_iota` loop) for production-scale runs creates
// hundreds of thousands of coins and can take minutes. This cache lets a
// second run reuse the coins from a previous run, skipping the slow
// generation step.

use std::{path::Path, str::FromStr, sync::Arc};

use anyhow::{Context, Result, anyhow};
use fastcrypto::traits::{EncodeDecodeBase64, KeyPair};
use iota_types::{
    base_types::{IotaAddress, ObjectID, ObjectRef, SequenceNumber},
    crypto::{AccountKeyPair, IotaKeyPair},
    digests::ObjectDigest,
};
use serde::{Deserialize, Serialize};
use tracing::{info, warn};

use crate::{ValidatorProxy, workloads::Gas};

#[derive(Serialize, Deserialize)]
pub struct CachedGas {
    pub address: String,
    pub object_id: String,
    pub version: u64,
    pub digest: String,
    /// IotaKeyPair-encoded base64 string (`iotaprivkey1…`-style). Stored
    /// rather than the raw Ed25519 bytes so we can round-trip through
    /// `IotaKeyPair::Ed25519(_)`.
    pub keypair_b64: String,
}

#[derive(Serialize, Deserialize)]
pub struct CachedWorkload {
    pub init_gas: Vec<CachedGas>,
    pub payload_gas: Vec<CachedGas>,
}

#[derive(Serialize, Deserialize)]
pub struct CachedPool {
    pub version: u32,
    pub config_hash: String,
    pub primary_owner: String,
    pub workloads: Vec<CachedWorkload>,
}

/// Hash of the inputs that affect coin-pool shape. If any of these change
/// between runs, the cached pool is invalidated and a fresh setup runs.
pub fn config_hash(
    primary_owner: IotaAddress,
    target_qps: &[u64],
    in_flight_ratio: &[u64],
    num_workers: &[u64],
    num_transfer_accounts: u64,
) -> String {
    use std::hash::{DefaultHasher, Hash, Hasher};
    let mut h = DefaultHasher::new();
    primary_owner.to_string().hash(&mut h);
    target_qps.hash(&mut h);
    in_flight_ratio.hash(&mut h);
    num_workers.hash(&mut h);
    num_transfer_accounts.hash(&mut h);
    format!("{:016x}", h.finish())
}

fn to_cached(g: &Gas) -> CachedGas {
    let (obj_ref, addr, kp) = g;
    let iota_kp = IotaKeyPair::Ed25519(kp.as_ref().copy());
    CachedGas {
        address: addr.to_string(),
        object_id: obj_ref.object_id.to_string(),
        version: obj_ref.version.as_u64(),
        digest: obj_ref.digest.to_string(),
        keypair_b64: iota_kp.encode_base64(),
    }
}

fn from_cached(c: &CachedGas) -> Result<Gas> {
    let addr = IotaAddress::from_str(&c.address)
        .map_err(|e| anyhow!("invalid address {}: {e}", c.address))?;
    let object_id = ObjectID::from_str(&c.object_id)
        .map_err(|e| anyhow!("invalid object_id {}: {e}", c.object_id))?;
    let version = SequenceNumber::from_u64(c.version);
    let digest = ObjectDigest::from_str(&c.digest)
        .map_err(|e| anyhow!("invalid digest {}: {e}", c.digest))?;
    let iota_kp = IotaKeyPair::decode_base64(&c.keypair_b64)
        .map_err(|e| anyhow!("invalid keypair b64: {e}"))?;
    let kp: AccountKeyPair = match iota_kp {
        IotaKeyPair::Ed25519(kp) => kp,
        other => return Err(anyhow!("expected Ed25519 keypair, got {:?}", other)),
    };
    Ok((
        ObjectRef::new(object_id, version, digest),
        addr,
        Arc::new(kp),
    ))
}

pub fn save(path: &Path, pool: &CachedPool) -> Result<()> {
    let json = serde_json::to_string_pretty(pool)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, json).with_context(|| format!("write {}", path.display()))?;
    info!("Gas pool cache saved to {}", path.display());
    Ok(())
}

pub fn load(path: &Path) -> Result<CachedPool> {
    let raw = std::fs::read_to_string(path).with_context(|| format!("read {}", path.display()))?;
    let pool: CachedPool =
        serde_json::from_str(&raw).with_context(|| format!("parse {}", path.display()))?;
    Ok(pool)
}

/// Verify each cached coin still exists on-chain. If any missing, the cache
/// is stale (epoch advanced, coins reused, etc.) and the caller should
/// regenerate.
pub async fn verify_all_exist(
    proxy: &Arc<dyn ValidatorProxy + Send + Sync>,
    pool: &CachedPool,
) -> bool {
    use std::collections::HashSet;
    // Group object IDs by owner address so we can do one query per owner.
    let mut by_owner: std::collections::HashMap<IotaAddress, HashSet<ObjectID>> =
        Default::default();
    for w in &pool.workloads {
        for c in w.init_gas.iter().chain(w.payload_gas.iter()) {
            let addr = match IotaAddress::from_str(&c.address) {
                Ok(a) => a,
                Err(_) => return false,
            };
            let oid = match ObjectID::from_str(&c.object_id) {
                Ok(o) => o,
                Err(_) => return false,
            };
            by_owner.entry(addr).or_default().insert(oid);
        }
    }
    for (addr, expected) in by_owner {
        let owned = match proxy.get_owned_objects(addr).await {
            Ok(v) => v.into_iter().map(|(_, o)| o.id()).collect::<HashSet<_>>(),
            Err(e) => {
                warn!("cache verify: failed to query owned objects for {addr}: {e}");
                return false;
            }
        };
        let missing = expected.difference(&owned).count();
        if missing > 0 {
            warn!(
                "cache verify: {addr} has {missing} cached coins no longer owned (out of {})",
                expected.len()
            );
            return false;
        }
    }
    true
}

/// Convert a `CachedPool` into the per-workload `(init_gas, payload_gas)`
/// pairs the bench-driver expects.
pub fn restore(pool: &CachedPool) -> Result<Vec<(Vec<Gas>, Vec<Gas>)>> {
    pool.workloads
        .iter()
        .map(|w| {
            let init = w
                .init_gas
                .iter()
                .map(from_cached)
                .collect::<Result<Vec<_>>>()?;
            let payload = w
                .payload_gas
                .iter()
                .map(from_cached)
                .collect::<Result<Vec<_>>>()?;
            Ok((init, payload))
        })
        .collect()
}

pub fn build_from_workloads(
    config_hash: String,
    primary_owner: IotaAddress,
    per_workload: &[(Vec<Gas>, Vec<Gas>)],
) -> CachedPool {
    CachedPool {
        version: 1,
        config_hash,
        primary_owner: primary_owner.to_string(),
        workloads: per_workload
            .iter()
            .map(|(init, payload)| CachedWorkload {
                init_gas: init.iter().map(to_cached).collect(),
                payload_gas: payload.iter().map(to_cached).collect(),
            })
            .collect(),
    }
}
