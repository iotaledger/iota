// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::fmt::{self};

use serde::{Deserialize, Serialize};

use crate::ObjectID;

pub type ExecutionStatus = iota_sdk_types::ExecutionStatus;
pub type ExecutionFailureStatus = iota_sdk_types::ExecutionError;
pub type PackageUpgradeError = iota_sdk_types::PackageUpgradeError;
pub type CommandArgumentError = iota_sdk_types::CommandArgumentError;
pub type TypeArgumentError = iota_sdk_types::TypeArgumentError;
pub type MoveLocation = iota_sdk_types::MoveLocation;
pub type MoveLocationOpt = iota_sdk_types::MoveLocationOpt;

#[cfg(test)]
#[path = "unit_tests/execution_status_tests.rs"]
mod execution_status_tests;

#[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize)]
pub struct CongestedObjects(pub Vec<ObjectID>);

impl fmt::Display for CongestedObjects {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> Result<(), fmt::Error> {
        for obj in &self.0 {
            write!(f, "{obj}, ")?;
        }
        Ok(())
    }
}

// #[derive(Eq, PartialEq, Clone, Debug, Serialize, Deserialize, Error,
// EnumVariantOrder)] pub enum ExecutionFailureStatus {
//     // General transaction errors
//     #[error("Insufficient Gas.")]
//     InsufficientGas,
//     #[error("Invalid Gas Object. Possibly not address-owned or possibly not
// an IOTA coin.")]     InvalidGasObject,
//     #[error("INVARIANT VIOLATION.")]
//     InvariantViolation,
//     #[error("Attempted to used feature that is not supported yet")]
//     FeatureNotYetSupported,
//     #[error(
//         "Move object with size {object_size} is larger \
//         than the maximum object size {max_object_size}"
//     )]
//     MoveObjectTooBig {
//         object_size: u64,
//         max_object_size: u64,
//     },
//     #[error(
//         "Move package with size {object_size} is larger than the \
//         maximum object size {max_object_size}"
//     )]
//     MovePackageTooBig {
//         object_size: u64,
//         max_object_size: u64,
//     },
//     #[error("Circular Object Ownership, including object {object}.")]
//     CircularObjectOwnership { object: ObjectID },

//     // Coin errors
//     #[error("Insufficient coin balance for operation.")]
//     InsufficientCoinBalance,
//     #[error("The coin balance overflows u64")]
//     CoinBalanceOverflow,

//     // Publish/Upgrade errors
//     #[error(
//         "Publish Error, Non-zero Address. \
//         The modules in the package must have their self-addresses set to
// zero."     )]
//     PublishErrorNonZeroAddress,

//     #[error(
//         "IOTA Move Bytecode Verification Error. \
//         Please run the IOTA Move Verifier for more information."
//     )]
//     IotaMoveVerificationError,

//     // Errors from the Move VM
//     //
//     // Indicates an error from a non-abort instruction
//     #[error(
//         "Move Primitive Runtime Error. Location: {0}. \
//         Arithmetic error, stack overflow, max value depth, etc."
//     )]
//     MovePrimitiveRuntimeError(MoveLocationOpt),
//     #[error("Move Runtime Abort. Location: {0}, Abort Code: {1}")]
//     MoveAbort(MoveLocation, u64),
//     #[error(
//         "Move Bytecode Verification Error. \
//         Please run the Bytecode Verifier for more information."
//     )]
//     VMVerificationOrDeserializationError,
//     #[error("MOVE VM INVARIANT VIOLATION.")]
//     VMInvariantViolation,

//     // Programmable Transaction Errors
//     #[error("Function Not Found.")]
//     FunctionNotFound,
//     #[error(
//         "Arity mismatch for Move function. \
//         The number of arguments does not match the number of parameters"
//     )]
//     ArityMismatch,
//     #[error(
//         "Type arity mismatch for Move function. \
//         Mismatch between the number of actual versus expected type
// arguments."     )]
//     TypeArityMismatch,
//     #[error("Non Entry Function Invoked. Move Call must start with an entry
// function")]     NonEntryFunctionInvoked,
//     #[error("Invalid command argument at {arg_idx}. {kind}")]
//     CommandArgumentError {
//         arg_idx: u16,
//         kind: CommandArgumentError,
//     },
//     #[error("Error for type argument at index {argument_idx}: {kind}")]
//     TypeArgumentError {
//         argument_idx: TypeParameterIndex,
//         kind: TypeArgumentError,
//     },
//     #[error(
//         "Unused result without the drop ability. \
//         Command result {result_idx}, return value {secondary_idx}"
//     )]
//     UnusedValueWithoutDrop { result_idx: u16, secondary_idx: u16 },
//     #[error(
//         "Invalid public Move function signature. \
//         Unsupported return type for return value {idx}"
//     )]
//     InvalidPublicFunctionReturnType { idx: u16 },
//     #[error("Invalid Transfer Object, object does not have public
// transfer.")]     InvalidTransferObject,

//     // Post-execution errors
//     //
//     // Indicates the effects from the transaction are too large
//     #[error(
//         "Effects of size {current_size} bytes too large. \
//     Limit is {max_size} bytes"
//     )]
//     EffectsTooLarge { current_size: u64, max_size: u64 },

//     #[error(
//         "Publish/Upgrade Error, Missing dependency. \
//          A dependency of a published or upgraded package has not been
// assigned an on-chain \          address."
//     )]
//     PublishUpgradeMissingDependency,

//     #[error(
//         "Publish/Upgrade Error, Dependency downgrade. \
//          Indirect (transitive) dependency of published or upgraded package
// has been assigned an \          on-chain version that is less than the
// version required by one of the package's \          transitive dependencies."
//     )]
//     PublishUpgradeDependencyDowngrade,

//     #[error("Invalid package upgrade. {upgrade_error}")]
//     PackageUpgradeError { upgrade_error: PackageUpgradeError },

//     // Indicates the transaction tried to write objects too large to storage
//     #[error(
//         "Written objects of {current_size} bytes too large. \
//     Limit is {max_size} bytes"
//     )]
//     WrittenObjectsTooLarge { current_size: u64, max_size: u64 },

//     #[error("Certificate is on the deny list")]
//     CertificateDenied,

//     #[error(
//         "IOTA Move Bytecode Verification Timeout. \
//         Please run the IOTA Move Verifier for more information."
//     )]
//     IotaMoveVerificationTimeout,

//     #[error("The shared object operation is not allowed.")]
//     SharedObjectOperationNotAllowed,

//     #[error("Certificate cannot be executed due to a dependency on a deleted
// shared object")]     InputObjectDeleted,

//     #[error("Certificate is cancelled due to congestion on shared objects:
// {congested_objects}.")]     ExecutionCancelledDueToSharedObjectCongestion {
// congested_objects: CongestedObjects },

//     #[error("Address {address:?} is denied for coin {coin_type}")]
//     AddressDeniedForCoin {
//         address: IotaAddress,
//         coin_type: String,
//     },

//     #[error("Coin type is globally paused for use: {coin_type}")]
//     CoinTypeGlobalPause { coin_type: String },

//     #[error("Certificate is cancelled because randomness could not be
// generated this epoch")]     ExecutionCancelledDueToRandomnessUnavailable,

//     // Certificate is cancelled due to congestion on shared objects;
//     // suggested gas price can be used to give this certificate more
// priority.     #[error(
//         "Certificate is cancelled due to congestion on shared objects:
// {congested_objects}. \             To give this certificate more priority to
// be executed, its gas price can be increased \             to at least
// {suggested_gas_price}."     )]
//     ExecutionCancelledDueToSharedObjectCongestionV2 {
//         congested_objects: CongestedObjects,
//         suggested_gas_price: u64,
//     },

//     #[error("A valid linkage was unable to be determined for the
// transaction")]     InvalidLinkage,
//     // NOTE: if you want to add a new enum,
//     // please add it at the end for Rust SDK backward compatibility.
// }
