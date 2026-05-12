// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Move package.
//!
//! This module contains the [MovePackage] and types necessary for describing
//! its update behavior and linkage information for module resolution during
//! execution.
//!
//! Upgradeable packages form a version chain. This is simply the conceptual
//! chain of package versions, with their monotonically increasing version
//! numbers. Package { version: 1 } => Package { version: 2 } => ...
//!
//! The code contains terminology that may be confusing for the uninitiated,
//! like `Module ID`, `Package ID`, `Storage ID` and `Runtime ID`. For avoidance
//! of doubt these concepts are defined like so:
//! - `Package ID` is the [ObjectID] representing the address by which the given
//!   package may be found in storage.
//! - `Runtime ID` will always mean the `Package ID`/`Storage ID` of the
//!   initially published package. For a non upgradeable package this will
//!   always be equal to `Storage ID`. For an upgradeable package, it will be
//!   the `Storage ID` of the package's first deployed version.
//! - `Storage ID` is the `Package ID`, and it is mostly used in to highlight
//!   that we are talking about the current `Package ID` and not the `Runtime
//!   ID`
//! - `Module ID` is the the type
//!   [ModuleID](move_core_types::language_storage::ModuleId).
//!
//! Some of these are redundant and have overlapping meaning, so whenever
//! reasonable/necessary the possible naming will be listed. From all of these
//! `Runtime ID` and `Module ID` are the most confusing. `Module ID` may be used
//! with `Runtime ID` and `Storage ID` depending on the context. While `Runtime
//! ID` is mostly used in name resolution during runtime, when a package with
//! its modules has been loaded.

use std::{
    collections::{BTreeMap, BTreeSet},
    hash::Hash,
};

use derive_more::Display;
use iota_protocol_config::ProtocolConfig;
pub use iota_sdk_types::move_package::{MovePackage, TypeOrigin, UpgradeInfo};
use iota_sdk_types::{Identifier, Version};
use move_binary_format::{
    binary_config::BinaryConfig, file_format::CompiledModule, file_format_common::VERSION_6,
    normalized,
};
use serde::{Deserialize, Serialize};
use serde_with::{Bytes, serde_as};

use crate::{
    IotaAddress, TypeTag,
    base_types::{ObjectID, SequenceNumber, StructTag},
    collection_types::{Entry, VecMap},
    derived_object,
    error::{ExecutionError, ExecutionErrorKind, IotaError, IotaResult},
    execution_status::PackageUpgradeError,
    id::{ID, UID},
    iota_serde::TypeName,
};

pub const PACKAGE_METADATA_MODULE_NAME: Identifier = Identifier::from_static("package_metadata");
pub const PACKAGE_METADATA_V1_STRUCT_NAME: Identifier =
    Identifier::from_static("PackageMetadataV1");
pub const PACKAGE_METADATA_KEY_STRUCT_NAME: Identifier =
    Identifier::from_static("PackageMetadataKey");

#[derive(Clone, Debug)]
/// Additional information about a function
pub struct FnInfo {
    /// If true, it's a function involved in testing (`[test]`, `[test_only]`,
    /// `[expected_failure]`)
    pub is_test: bool,
    /// If set, function was marked to represent authenticator function of
    /// given version.
    pub authenticator_version: Option<u8>,
}

#[derive(Clone, Debug, Eq, Hash, PartialEq, PartialOrd, Ord)]
/// Uniquely identifies a function in a module
pub struct FnInfoKey {
    pub fn_name: String,
    pub mod_name: String,
    pub mod_addr: IotaAddress,
}

/// A map from function info keys to function info
pub type FnInfoMap = BTreeMap<FnInfoKey, FnInfo>;

// NB: do _not_ add `Serialize` or `Deserialize` to this enum. Convert to u8
// first  or use the associated constants before storing in any serialization
// setting.
/// Rust representation of upgrade policy constants in `iota::package`.
#[repr(u8)]
#[derive(Display, Debug, Clone, Copy)]
pub enum UpgradePolicy {
    #[display("COMPATIBLE")]
    Compatible = 0,
    #[display("ADDITIVE")]
    Additive = 128,
    #[display("DEP_ONLY")]
    DepOnly = 192,
}

impl UpgradePolicy {
    /// Convenience accessors to the upgrade policies as u8s.
    pub const COMPATIBLE: u8 = Self::Compatible as u8;
    pub const ADDITIVE: u8 = Self::Additive as u8;
    pub const DEP_ONLY: u8 = Self::DepOnly as u8;

    pub fn is_valid_policy(policy: &u8) -> bool {
        Self::try_from(*policy).is_ok()
    }
}

impl TryFrom<u8> for UpgradePolicy {
    type Error = ();
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            x if x == Self::Compatible as u8 => Ok(Self::Compatible),
            x if x == Self::Additive as u8 => Ok(Self::Additive),
            x if x == Self::DepOnly as u8 => Ok(Self::DepOnly),
            _ => Err(()),
        }
    }
}

/// Rust representation of `iota::package::UpgradeCap`.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpgradeCap {
    pub id: UID,
    pub package: ID,
    pub version: u64,
    pub policy: u8,
}

/// Rust representation of `iota::package::UpgradeTicket`.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpgradeTicket {
    pub cap: ID,
    pub package: ID,
    pub policy: u8,
    pub digest: Vec<u8>,
}

/// Rust representation of `iota::package::UpgradeReceipt`.
#[derive(Debug, Serialize, Deserialize)]
pub struct UpgradeReceipt {
    pub cap: ID,
    pub package: ID,
}

mod move_package_ext {
    pub trait Sealed {}
    impl Sealed for super::MovePackage {}
}

pub trait MovePackageExt: Sized + move_package_ext::Sealed {
    fn new_initial<'p>(
        modules: &[CompiledModule],
        protocol_config: &ProtocolConfig,
        transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<MovePackage, ExecutionError>;

    fn new_upgraded<'p>(
        &self,
        storage_id: ObjectID,
        modules: &[CompiledModule],
        protocol_config: &ProtocolConfig,
        transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<MovePackage, ExecutionError>;

    fn new_system(
        version: SequenceNumber,
        modules: &[CompiledModule],
        dependencies: impl IntoIterator<Item = ObjectID>,
    ) -> MovePackage;

    fn from_module_iter_with_type_origin_table<'p>(
        storage_id: ObjectID,
        self_id: ObjectID,
        version: SequenceNumber,
        modules: &[CompiledModule],
        protocol_config: &ProtocolConfig,
        type_origin_table: Vec<TypeOrigin>,
        transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<MovePackage, ExecutionError>;

    fn original_package_id(&self) -> ObjectID;

    fn deserialize_module(
        &self,
        module: &Identifier,
        binary_config: &BinaryConfig,
    ) -> IotaResult<CompiledModule>;

    fn normalize<S: Hash + Eq + Clone + ToString, Pool: normalized::StringPool<String = S>>(
        &self,
        pool: &mut Pool,
        binary_config: &BinaryConfig,
        include_code: bool,
    ) -> IotaResult<BTreeMap<String, normalized::Module<S>>>;
}

impl MovePackageExt for MovePackage {
    /// Create an initial version of the package along with this version's type
    /// origin and linkage tables.
    ///
    /// # Undefined behavior
    ///
    /// All passed modules must have the same `Runtime ID` or the behavior is
    /// undefined.
    fn new_initial<'p>(
        modules: &[CompiledModule],
        protocol_config: &ProtocolConfig,
        transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<MovePackage, ExecutionError> {
        let module = modules
            .first()
            .expect("Tried to build a Move package from an empty iterator of Compiled modules");
        let runtime_id = ObjectID::new(module.address().into_bytes());
        let storage_id = runtime_id;
        let type_origin_table = build_initial_type_origin_table(modules);

        MovePackage::from_module_iter_with_type_origin_table(
            storage_id,
            runtime_id,
            Version::OBJECT_START,
            modules,
            protocol_config,
            type_origin_table,
            transitive_dependencies,
        )
    }

    /// Create an upgraded version of the package along with this version's type
    /// origin and linkage tables.
    ///
    /// # Undefined behavior
    ///
    /// All passed modules must have the same `Runtime ID` or the behavior is
    /// undefined.
    fn new_upgraded<'p>(
        &self,
        storage_id: ObjectID,
        modules: &[CompiledModule],
        protocol_config: &ProtocolConfig,
        transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<MovePackage, ExecutionError> {
        let module = modules
            .first()
            .expect("Tried to build a Move package from an empty iterator of Compiled modules");
        let runtime_id = ObjectID::new(module.address().into_bytes());
        let type_origin_table = build_upgraded_type_origin_table(self, modules, storage_id)?;
        let mut new_version = self.version();
        new_version.increment().unwrap();

        MovePackage::from_module_iter_with_type_origin_table(
            storage_id,
            runtime_id,
            new_version,
            modules,
            protocol_config,
            type_origin_table,
            transitive_dependencies,
        )
    }

    fn new_system(
        version: SequenceNumber,
        modules: &[CompiledModule],
        dependencies: impl IntoIterator<Item = ObjectID>,
    ) -> MovePackage {
        let module = modules
            .first()
            .expect("Tried to build a Move package from an empty iterator of Compiled modules");

        let storage_id = ObjectID::new(module.address().into_bytes());
        let type_origin_table = build_initial_type_origin_table(modules);

        let linkage_table = BTreeMap::from_iter(dependencies.into_iter().map(|dep| {
            let info = UpgradeInfo {
                upgraded_id: dep,
                // The upgraded version is used by other packages that transitively depend on this
                // system package, to make sure that if they choose a different version to depend on
                // compared to their dependencies, they pick a greater version.
                //
                // However, in the case of system packages, although they can be upgraded, unlike
                // other packages, only one version can be in use on the network at any given time,
                // so it is not possible for a package to require a different system package version
                // compared to its dependencies.
                //
                // This reason, coupled with the fact that system packages can only depend on each
                // other, mean that their own linkage tables always report a version of zero.
                upgraded_version: SequenceNumber::default(),
            };
            (dep, info)
        }));

        let module_map = BTreeMap::from_iter(modules.iter().map(|module| {
            let name = Identifier::new_unchecked(module.name().as_str());
            let mut bytes = Vec::new();
            module
                .serialize_with_version(module.version, &mut bytes)
                .unwrap();
            (name, bytes)
        }));

        MovePackage::new(
            storage_id,
            version,
            module_map,
            u64::MAX, // System packages are not subject to the size limit
            type_origin_table,
            linkage_table,
        )
        .expect("System packages are not subject to a size limit")
    }

    fn from_module_iter_with_type_origin_table<'p>(
        storage_id: ObjectID,
        self_id: ObjectID,
        version: SequenceNumber,
        modules: &[CompiledModule],
        protocol_config: &ProtocolConfig,
        type_origin_table: Vec<TypeOrigin>,
        transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    ) -> Result<MovePackage, ExecutionError> {
        let mut module_map = BTreeMap::new();
        let mut immediate_dependencies = BTreeSet::new();

        for module in modules {
            let name = Identifier::new_unchecked(module.name().as_str());

            immediate_dependencies.extend(
                module
                    .immediate_dependencies()
                    .into_iter()
                    .map(|dep| ObjectID::new(dep.address().into_bytes())),
            );

            let mut bytes = Vec::new();
            let version = if protocol_config.move_binary_format_version() > VERSION_6 {
                module.version
            } else {
                VERSION_6
            };
            module.serialize_with_version(version, &mut bytes).unwrap();
            module_map.insert(name, bytes);
        }

        immediate_dependencies.remove(&self_id);
        let linkage_table = build_linkage_table(
            immediate_dependencies,
            transitive_dependencies,
            protocol_config,
        )?;

        Ok(MovePackage::new(
            storage_id,
            version,
            module_map,
            protocol_config.max_move_package_size(),
            type_origin_table,
            linkage_table,
        )?)
    }

    /// The `Package ID` of the first version of this package.
    ///
    /// Also referred to as `Runtime ID`.
    ///
    /// Regardless of which version of the package we are working with, this
    /// function will always return the `Package ID`/`Storage ID` of the first
    /// package version in the version chain.
    fn original_package_id(&self) -> ObjectID {
        if self.version == SequenceNumber::OBJECT_START {
            // for a non-upgraded package, original ID is just the package ID
            return self.id;
        }

        let bytes = self.modules.values().next().expect("Empty module map");
        // Remember, that all modules will contain the `Package ID` of the first
        // deployed package. This is why taking any of them will produce the
        // original package id.
        let module = CompiledModule::deserialize_with_defaults(bytes)
            .expect("A Move package contains a module that cannot be deserialized");
        ObjectID::new(module.address().into_bytes())
    }

    fn deserialize_module(
        &self,
        module: &Identifier,
        binary_config: &BinaryConfig,
    ) -> IotaResult<CompiledModule> {
        // TODO use the session's cache
        let bytes =
            self.serialized_module_map()
                .get(module)
                .ok_or_else(|| IotaError::ModuleNotFound {
                    module_name: module.to_string(),
                })?;

        CompiledModule::deserialize_with_config(bytes, binary_config).map_err(|error| {
            IotaError::ModuleDeserializationFailure {
                error: error.to_string(),
            }
        })
    }

    /// If `include_code` is set to `false`, the normalized module will skip
    /// function bodies but still include the signatures.
    fn normalize<S: Hash + Eq + Clone + ToString, Pool: normalized::StringPool<String = S>>(
        &self,
        pool: &mut Pool,
        binary_config: &BinaryConfig,
        include_code: bool,
    ) -> IotaResult<BTreeMap<String, normalized::Module<S>>> {
        normalize_modules(pool, self.modules.values(), binary_config, include_code)
    }
}

impl UpgradeCap {
    /// Create an `UpgradeCap` for the newly published package at `package_id`,
    /// and associate it with the fresh `uid`.
    pub fn new(uid: ObjectID, package_id: ObjectID) -> Self {
        UpgradeCap {
            id: UID::new(uid),
            package: ID::new(package_id),
            version: 1,
            policy: UpgradePolicy::COMPATIBLE,
        }
    }
}

impl UpgradeReceipt {
    /// Create an `UpgradeReceipt` for the upgraded package at `package_id`
    /// using the `UpgradeTicket` and newly published package id.
    pub fn new(upgrade_ticket: UpgradeTicket, upgraded_package_id: ObjectID) -> Self {
        UpgradeReceipt {
            cap: upgrade_ticket.cap,
            package: ID::new(upgraded_package_id),
        }
    }
}

/// Checks if a function is annotated with one of the test-related annotations
pub fn is_test_fun(name: &str, module: &CompiledModule, fn_info_map: &FnInfoMap) -> bool {
    let mod_handle = module.self_handle();
    let mod_addr = IotaAddress::new(
        module
            .address_identifier_at(mod_handle.address)
            .into_bytes(),
    );
    let mod_name = module.name().to_string();
    let fn_info_key = FnInfoKey {
        fn_name: name.to_string(),
        mod_name,
        mod_addr,
    };
    match fn_info_map.get(&fn_info_key) {
        Some(fn_info) => fn_info.is_test,
        None => false,
    }
}

pub fn get_authenticator_version_from_fun(
    name: &str,
    module: &CompiledModule,
    fn_info_map: &FnInfoMap,
) -> Option<u8> {
    let mod_handle = module.self_handle();
    let mod_addr = IotaAddress::from(
        module
            .address_identifier_at(mod_handle.address)
            .into_bytes(),
    );
    let mod_name = module.name().to_string();
    let fn_info_key = FnInfoKey {
        fn_name: name.to_string(),
        mod_name,
        mod_addr,
    };
    match fn_info_map.get(&fn_info_key) {
        Some(FnInfo {
            is_test: _,
            authenticator_version: Some(v),
        }) => Some(*v),
        _ => None,
    }
}

/// If `include_code` is set to `false`, the normalized module will skip
/// function bodies but still include the signatures.
pub fn normalize_modules<
    'a,
    S: Hash + Eq + Clone + ToString,
    Pool: normalized::StringPool<String = S>,
    I,
>(
    pool: &mut Pool,
    modules: I,
    binary_config: &BinaryConfig,
    include_code: bool,
) -> IotaResult<BTreeMap<String, normalized::Module<S>>>
where
    I: Iterator<Item = &'a Vec<u8>>,
{
    let mut normalized_modules = BTreeMap::new();
    for bytecode in modules {
        let module =
            CompiledModule::deserialize_with_config(bytecode, binary_config).map_err(|error| {
                IotaError::ModuleDeserializationFailure {
                    error: error.to_string(),
                }
            })?;
        let normalized_module = normalized::Module::new(pool, &module, include_code);
        normalized_modules.insert(normalized_module.name().to_string(), normalized_module);
    }
    Ok(normalized_modules)
}

/// If `include_code` is set to `false`, the normalized module will skip
/// function bodies but still include the signatures.
pub fn normalize_deserialized_modules<
    'a,
    S: Hash + Eq + Clone + ToString,
    Pool: normalized::StringPool<String = S>,
    I,
>(
    pool: &mut Pool,
    modules: I,
    include_code: bool,
) -> BTreeMap<String, normalized::Module<S>>
where
    I: Iterator<Item = &'a CompiledModule>,
{
    let mut normalized_modules = BTreeMap::new();
    for module in modules {
        let normalized_module = normalized::Module::new(pool, module, include_code);
        normalized_modules.insert(normalized_module.name().to_string(), normalized_module);
    }
    normalized_modules
}

fn build_linkage_table<'p>(
    mut immediate_dependencies: BTreeSet<ObjectID>,
    transitive_dependencies: impl IntoIterator<Item = &'p MovePackage>,
    protocol_config: &ProtocolConfig,
) -> Result<BTreeMap<ObjectID, UpgradeInfo>, ExecutionError> {
    let mut linkage_table = BTreeMap::new();
    let mut dep_linkage_tables = vec![];

    for transitive_dep in transitive_dependencies.into_iter() {
        // original_package_id will deserialize a module but only for the purpose of
        // obtaining "original ID" of the package containing it so using max
        // Move binary version during deserialization is OK
        let original_id = MovePackage::original_package_id(transitive_dep);

        let imm_dep = immediate_dependencies.remove(&original_id);

        if protocol_config.dependency_linkage_error() {
            dep_linkage_tables.push(&transitive_dep.linkage_table);

            let existing = linkage_table.insert(
                original_id,
                UpgradeInfo {
                    upgraded_id: transitive_dep.id,
                    upgraded_version: transitive_dep.version,
                },
            );

            if existing.is_some() {
                return Err(ExecutionErrorKind::InvalidLinkage.into());
            }
        } else {
            if imm_dep {
                // Found an immediate dependency, mark it as seen, and stash a reference to its
                // linkage table to check later.
                dep_linkage_tables.push(&transitive_dep.linkage_table);
            }
            linkage_table.insert(
                original_id,
                UpgradeInfo {
                    upgraded_id: transitive_dep.id,
                    upgraded_version: transitive_dep.version,
                },
            );
        }
    }
    // (1) Every dependency is represented in the transitive dependencies
    if !immediate_dependencies.is_empty() {
        return Err(ExecutionErrorKind::PublishUpgradeMissingDependency.into());
    }

    // (2) Every dependency's linkage table is superseded by this linkage table
    for dep_linkage_table in dep_linkage_tables {
        for (original_id, dep_info) in dep_linkage_table {
            let Some(our_info) = linkage_table.get(original_id) else {
                return Err(ExecutionErrorKind::PublishUpgradeMissingDependency.into());
            };

            if our_info.upgraded_version < dep_info.upgraded_version {
                return Err(ExecutionErrorKind::PublishUpgradeDependencyDowngrade.into());
            }
        }
    }

    Ok(linkage_table)
}

fn build_initial_type_origin_table(modules: &[CompiledModule]) -> Vec<TypeOrigin> {
    modules
        .iter()
        .flat_map(|m| {
            m.struct_defs()
                .iter()
                .map(|struct_def| {
                    let struct_handle = m.datatype_handle_at(struct_def.struct_handle);
                    let module_name = m.name().to_string();
                    let struct_name = m.identifier_at(struct_handle.name).to_string();
                    let package = ObjectID::new(m.self_id().address().into_bytes());
                    TypeOrigin {
                        module_name: Identifier::new_unchecked(module_name),
                        datatype_name: Identifier::new_unchecked(struct_name),
                        package,
                    }
                })
                .chain(m.enum_defs().iter().map(|enum_def| {
                    let enum_handle = m.datatype_handle_at(enum_def.enum_handle);
                    let module_name = m.name().to_string();
                    let enum_name = m.identifier_at(enum_handle.name).to_string();
                    let package = ObjectID::new(m.self_id().address().into_bytes());
                    TypeOrigin {
                        module_name: Identifier::new_unchecked(module_name),
                        datatype_name: Identifier::new_unchecked(enum_name),
                        package,
                    }
                }))
        })
        .collect()
}

fn build_upgraded_type_origin_table(
    predecessor: &MovePackage,
    modules: &[CompiledModule],
    storage_id: ObjectID,
) -> Result<Vec<TypeOrigin>, ExecutionError> {
    let mut new_table = vec![];
    let mut existing_table = predecessor.type_origin_map();
    for m in modules {
        for struct_def in m.struct_defs() {
            let struct_handle = m.datatype_handle_at(struct_def.struct_handle);
            let module_name = Identifier::new_unchecked(m.name().as_str());
            let struct_name =
                Identifier::new_unchecked(m.identifier_at(struct_handle.name).as_str());
            let mod_key = (module_name.clone(), struct_name.clone());
            // if id exists in the predecessor's table, use it, otherwise use the id of the
            // upgraded module
            let package = existing_table.remove(&mod_key).unwrap_or(storage_id);
            new_table.push(TypeOrigin {
                module_name,
                datatype_name: struct_name,
                package,
            });
        }

        for enum_def in m.enum_defs() {
            let enum_handle = m.datatype_handle_at(enum_def.enum_handle);
            let module_name = Identifier::new_unchecked(m.name().as_str());
            let enum_name = Identifier::new_unchecked(m.identifier_at(enum_handle.name).as_str());
            let mod_key = (module_name.clone(), enum_name.clone());
            // if id exists in the predecessor's table, use it, otherwise use the id of the
            // upgraded module
            let package = existing_table.remove(&mod_key).unwrap_or(storage_id);
            new_table.push(TypeOrigin {
                module_name,
                datatype_name: enum_name,
                package,
            });
        }
    }

    if !existing_table.is_empty() {
        Err(ExecutionError::from_kind(
            ExecutionErrorKind::PackageUpgradeError {
                kind: PackageUpgradeError::IncompatibleUpgrade,
            },
        ))
    } else {
        Ok(new_table)
    }
}

/// IOTA specific metadata attached to the metadata section of file_format.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RuntimeModuleMetadataWrapper {
    pub version: u64,
    #[serde_as(as = "Bytes")]
    pub inner: Vec<u8>,
}

impl RuntimeModuleMetadataWrapper {
    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        // Safe unwrap as the RuntimeModuleMetadataWrapper struct is always serializable
        bcs::to_bytes(&self).unwrap()
    }
}

impl From<RuntimeModuleMetadata> for RuntimeModuleMetadataWrapper {
    fn from(metadata: RuntimeModuleMetadata) -> Self {
        match metadata {
            RuntimeModuleMetadata::V1(inner) => RuntimeModuleMetadataWrapper {
                version: 1,
                inner: inner.to_bcs_bytes(),
            },
        }
    }
}

/// IOTA specific metadata attached to the metadata section of file_format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum RuntimeModuleMetadata {
    V1(RuntimeModuleMetadataV1),
}

impl RuntimeModuleMetadata {
    pub fn add_function_attribute(&mut self, function_name: String, attribute: IotaAttribute) {
        match self {
            RuntimeModuleMetadata::V1(metadata) => {
                metadata.add_function_attribute(function_name, attribute)
            }
        }
    }

    pub fn is_empty(&self) -> bool {
        match self {
            RuntimeModuleMetadata::V1(metadata) => metadata.is_empty(),
        }
    }

    pub fn fun_attributes_iter(
        &self,
    ) -> Box<dyn Iterator<Item = (&String, &Vec<IotaAttribute>)> + '_> {
        match self {
            RuntimeModuleMetadata::V1(metadata) => Box::new(metadata.fun_attributes.iter()),
        }
    }
}

impl Default for RuntimeModuleMetadata {
    fn default() -> Self {
        RuntimeModuleMetadata::V1(RuntimeModuleMetadataV1::default())
    }
}

impl TryFrom<RuntimeModuleMetadataWrapper> for RuntimeModuleMetadata {
    type Error = IotaError;

    fn try_from(wrapper: RuntimeModuleMetadataWrapper) -> Result<Self, Self::Error> {
        match wrapper.version {
            1 => {
                let inner: RuntimeModuleMetadataV1 =
                    bcs::from_bytes(&wrapper.inner).map_err(|e| {
                        IotaError::RuntimeModuleMetadataDeserialization {
                            error: e.to_string(),
                        }
                    })?;
                Ok(RuntimeModuleMetadata::V1(inner))
            }
            _ => Err(IotaError::RuntimeModuleMetadataDeserialization {
                error: format!(
                    "Unsupported runtime module metadata version: {}",
                    wrapper.version
                ),
            }),
        }
    }
}

/// The list of iota attribute types recognized by the compiler.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub enum IotaAttribute {
    Authenticator(AuthenticatorAttribute),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct AuthenticatorAttribute {
    pub version: u8,
}

impl IotaAttribute {
    pub fn authenticator_attribute(version: u8) -> Self {
        IotaAttribute::Authenticator(AuthenticatorAttribute { version })
    }
}

/// V1 of IOTA specific metadata.
#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RuntimeModuleMetadataV1 {
    /// Attributes attached to functions, by definition index.
    pub fun_attributes: BTreeMap<String, Vec<IotaAttribute>>,
}

impl RuntimeModuleMetadataV1 {
    pub fn add_function_attribute(&mut self, function_name: String, attribute: IotaAttribute) {
        self.fun_attributes
            .entry(function_name)
            .or_default()
            .push(attribute);
    }

    pub fn is_empty(&self) -> bool {
        self.fun_attributes.is_empty()
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        // Safe unwrap as the RuntimeModuleMetadataV1 struct is always serializable
        bcs::to_bytes(&self).unwrap()
    }
}

/// Enum for handling the PackageMetadata framework type. The PackageMetadata is
/// IOTA specific metadata derived from a package and readable on-chain. This
/// enums helps with the versioning, which is actually used as the object
/// content, i.e., PackageMetadataV1 is the type used on-chain.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum PackageMetadata {
    V1(PackageMetadataV1),
}

impl PackageMetadata {
    /// Create a `PackageMetadata` for the newly
    /// published/upgraded package at `package_id`
    pub fn new_v1(
        uid: ObjectID,
        storage_id: ObjectID,
        runtime_id: ObjectID,
        package_version: u64,
        modules_metadata_map: BTreeMap<String, BTreeMap<String, TypeTag>>,
    ) -> Self {
        PackageMetadata::V1(PackageMetadataV1::new(
            uid,
            storage_id,
            runtime_id,
            package_version,
            modules_metadata_map,
        ))
    }

    pub fn type_(&self) -> StructTag {
        match self {
            PackageMetadata::V1(_) => PackageMetadataV1::type_(),
        }
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        match self {
            PackageMetadata::V1(inner) => inner.to_bcs_bytes(),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize, Clone, Eq, PartialEq)]
pub struct PackageMetadataKey {
    // This field is required to make a Rust struct compatible with an empty Move one.
    // An empty Move struct contains a 1-byte dummy bool field because empty fields are not
    // allowed in the bytecode.
    dummy_field: bool,
}

impl PackageMetadataKey {
    pub fn tag() -> StructTag {
        StructTag::new(
            IotaAddress::FRAMEWORK,
            PACKAGE_METADATA_MODULE_NAME,
            PACKAGE_METADATA_KEY_STRUCT_NAME,
            Vec::new(),
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        // Safe unwrap as the PackageMetadataKey struct is always serializable
        bcs::to_bytes(&self).unwrap()
    }
}

pub fn derive_package_metadata_id(package_storage_id: ObjectID) -> ObjectID {
    derived_object::derive_object_id(
        package_storage_id,
        &PackageMetadataKey::tag().into(),
        &PackageMetadataKey::default().to_bcs_bytes(),
    )
    .unwrap() // safe because type tag is known
}

/// V1 of IOTA specific package metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PackageMetadataV1 {
    // The package metadata object UID
    pub uid: UID,
    /// Storage ID of the package represented by this metadata
    /// The object id of the runtime package metadata object is derived from
    /// this value.
    pub storage_id: ID,
    /// Runtime ID of the package represented by this metadata. Runtime ID is
    /// the Storage ID of the first version of a package.
    pub runtime_id: ID,
    /// Version of the package represented by this metadata
    pub package_version: u64,
    // Handles to internal package modules
    pub modules_metadata: VecMap<String, ModuleMetadataV1>,
}

impl PackageMetadataV1 {
    fn new(
        uid: ObjectID,
        storage_id: ObjectID,
        runtime_id: ObjectID,
        package_version: u64,
        modules_metadata_map: BTreeMap<String, BTreeMap<String, TypeTag>>,
    ) -> Self {
        let mut modules_metadata = VecMap { contents: vec![] };

        for (module_name, module_metadata_map) in modules_metadata_map {
            let mut module_metadata = ModuleMetadataV1 {
                authenticator_metadata: vec![],
            };
            for (function_name, account_type) in module_metadata_map {
                module_metadata
                    .authenticator_metadata
                    .push(AuthenticatorMetadataV1 {
                        function_name,
                        account_type,
                    });
            }
            modules_metadata.contents.push(Entry {
                key: module_name,
                value: module_metadata,
            });
        }

        Self {
            uid: UID::new(uid),
            storage_id: ID::new(storage_id),
            runtime_id: ID::new(runtime_id),
            package_version,
            modules_metadata,
        }
    }

    pub fn type_() -> StructTag {
        StructTag::new(
            IotaAddress::FRAMEWORK,
            PACKAGE_METADATA_MODULE_NAME,
            PACKAGE_METADATA_V1_STRUCT_NAME,
            vec![],
        )
    }

    pub fn to_bcs_bytes(&self) -> Vec<u8> {
        // Safe unwrap as the PackageMetadataV1 struct is always serializable
        bcs::to_bytes(&self).unwrap()
    }
}

/// V1 of IOTA specific module metadata. Only includes authenticator info.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleMetadataV1 {
    pub authenticator_metadata: Vec<AuthenticatorMetadataV1>,
}

impl ModuleMetadataV1 {
    pub fn is_empty(&self) -> bool {
        self.authenticator_metadata.is_empty()
    }
}

/// V1 of IOTA specific authenticator info metadata.
#[serde_as]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthenticatorMetadataV1 {
    pub function_name: String,
    #[serde_as(as = "TypeName")]
    pub account_type: TypeTag,
}
