// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_sdk_types::{Address, Command, Digest, ObjectId, TransactionExpiration, Version};

pub(crate) type OptionReadableDisplay =
    ::serde_with::As<Option<::serde_with::IfIsHumanReadable<::serde_with::DisplayFromStr>>>;

// A potentially unresolved user transaction
#[derive(serde::Serialize, serde::Deserialize)]
pub struct UnresolvedTransaction {
    #[serde(flatten)]
    pub ptb: UnresolvedProgrammableTransaction,
    pub sender: Address,
    pub gas_payment: Option<UnresolvedGasPayment>,
    pub expiration: TransactionExpiration,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UnresolvedProgrammableTransaction {
    pub inputs: Vec<UnresolvedInputArgument>,
    pub commands: Vec<Command>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UnresolvedGasPayment {
    pub objects: Vec<UnresolvedObjectReference>,
    pub owner: Address,
    #[serde(with = "OptionReadableDisplay")]
    pub price: Option<u64>,
    #[serde(with = "OptionReadableDisplay")]
    pub budget: Option<u64>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct UnresolvedObjectReference {
    pub object_id: ObjectId,
    #[serde(with = "OptionReadableDisplay")]
    pub version: Option<Version>,
    pub digest: Option<Digest>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum UnresolvedInputArgument {
    // contains no structs or objects
    Pure {
        #[serde(with = "::serde_with::As::<::serde_with::Bytes>")]
        value: Vec<u8>,
    },
    // A Move object, either immutable, or owned mutable.
    ImmutableOrOwned(UnresolvedObjectReference),
    // A Move object that's shared.
    // SharedObject::mutable controls whether caller asks for a mutable reference to shared
    // object.
    Shared {
        object_id: ObjectId,
        #[serde(with = "OptionReadableDisplay")]
        initial_shared_version: Option<u64>,
        mutable: Option<bool>,
    },
    // A Move object that can be received in this transaction.
    Receiving(UnresolvedObjectReference),
}
