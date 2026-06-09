// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::sync::Arc;

use iota_sdk_types::TypeTag;
use iota_types::base_types::IotaAddress;
use move_binary_format::errors::VMError;
use thiserror::Error;

#[derive(Error, Debug, Clone)]
pub enum Error {
    #[error("{0}")]
    Bcs(#[from] bcs::Error),

    #[error("Store {} error: {}", store, source)]
    Store {
        store: &'static str,
        source: Arc<dyn std::error::Error + Send + Sync + 'static>,
    },

    #[error("{0}")]
    Deserialize(VMError),

    #[error("Package has no modules: {0}")]
    EmptyPackage(IotaAddress),

    #[error("Function not found: {0}::{1}::{2}")]
    FunctionNotFound(IotaAddress, String, String),

    #[error("Conflicting types for input {0}: {1} and {2}")]
    InputTypeConflict(u16, TypeTag, TypeTag),

    #[error("Linkage not found for package: {0}")]
    LinkageNotFound(IotaAddress),

    #[error("Module not found: {0}::{1}")]
    ModuleNotFound(IotaAddress, String),

    #[error("No origin package found for {0}::{1}::{2}")]
    NoTypeOrigin(IotaAddress, String, String),

    #[error("Not a package: {0}")]
    NotAPackage(IotaAddress),

    #[error("Not an identifier: '{0}'")]
    NotAnIdentifier(String),

    #[error("Package not found: {0}")]
    PackageNotFound(IotaAddress),

    #[error("Datatype not found: {0}::{1}::{2}")]
    DatatypeNotFound(IotaAddress, String, String),

    #[error("More than {0} struct definitions required to resolve type")]
    TooManyTypeNodes(usize, usize),

    #[error("Expected at most {0} type parameters, got {1}")]
    TooManyTypeParams(usize, usize),

    #[error("Expected {0} type parameters, but got {1}")]
    TypeArityMismatch(usize, usize),

    #[error("Type parameter nesting exceeded limit of {0}")]
    TypeParamNesting(usize, usize),

    #[error("Type Parameter {0} out of bounds ({1})")]
    TypeParamOOB(u16, usize),

    #[error("Unexpected reference type.")]
    UnexpectedReference,

    #[error("Unexpected type: 'signer'.")]
    UnexpectedSigner,

    #[error("Unexpected error: {0}")]
    Unexpected(Arc<dyn std::error::Error + Send + Sync + 'static>),

    #[error("Type layout nesting exceeded limit of {0}")]
    ValueNesting(usize),
}
