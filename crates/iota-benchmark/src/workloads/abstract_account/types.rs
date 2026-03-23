// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::str::FromStr;

use anyhow::{Result, bail};
use serde::{Deserialize, Serialize};

use crate::workloads::abstract_account::AA_MODULE_NAME;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Default)]
pub enum AuthenticatorKind {
    #[default]
    Ed25519,
    Ed25519Heavy,
    HelloWorld,
    MaxArgs125,
}

impl AuthenticatorKind {
    pub fn module_name(&self) -> &'static str {
        AA_MODULE_NAME
    }

    pub fn function_name(&self) -> &'static str {
        match self {
            AuthenticatorKind::Ed25519 => "authenticate_ed25519",
            AuthenticatorKind::Ed25519Heavy => "authenticate_ed25519_heavy",
            AuthenticatorKind::HelloWorld => "authenticate_hello_world",
            AuthenticatorKind::MaxArgs125 => "authenticate_max_args_125",
        }
    }

    pub fn requires_bench_objects(&self) -> bool {
        matches!(self, AuthenticatorKind::MaxArgs125)
    }

    pub fn expected_bench_objects_count(&self) -> Option<usize> {
        match self {
            AuthenticatorKind::MaxArgs125 => Some(122),
            _ => None,
        }
    }
}

impl FromStr for AuthenticatorKind {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "ed25519" => Ok(AuthenticatorKind::Ed25519),
            "ed25519heavy" => Ok(AuthenticatorKind::Ed25519Heavy),
            "helloworld" => Ok(AuthenticatorKind::HelloWorld),
            "maxargs125" => Ok(AuthenticatorKind::MaxArgs125),
            _ => bail!("unknown AuthenticatorKind: {}", s),
        }
    }
}

impl std::fmt::Display for AuthenticatorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            AuthenticatorKind::Ed25519 => "ed25519",
            AuthenticatorKind::Ed25519Heavy => "ed25519heavy",
            AuthenticatorKind::HelloWorld => "helloworld",
            AuthenticatorKind::MaxArgs125 => "maxargs125",
        };
        f.write_str(s)
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Eq, PartialEq, Default)]
pub enum TxPayloadObjType {
    #[default]
    OwnedObject,
    SharedObject,
}

impl FromStr for TxPayloadObjType {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "owned-object" => Ok(TxPayloadObjType::OwnedObject),
            "shared-object" => Ok(TxPayloadObjType::SharedObject),
            _ => bail!("unknown TxPayloadObjType: {}", s),
        }
    }
}

impl std::fmt::Display for TxPayloadObjType {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        let s = match self {
            TxPayloadObjType::OwnedObject => "owned-object",
            TxPayloadObjType::SharedObject => "shared-object",
        };
        f.write_str(s)
    }
}
