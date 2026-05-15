// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::{
    base_types::ObjectID,
    collection_types::{Entry, VecMap},
    id::{ID, UID},
    move_package::{AuthenticatorMetadataV1, PackageMetadataV1},
    type_input::TypeName,
};
use move_core_types::language_storage::TypeTag;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct LegacyPackageMetadataV1 {
    uid: UID,
    storage_id: ID,
    runtime_id: ID,
    package_version: u64,
    modules_metadata: VecMap<String, LegacyModuleMetadataV1>,
}

#[derive(Debug, Serialize, Deserialize)]
struct LegacyModuleMetadataV1 {
    authenticator_metadata: Vec<AuthenticatorMetadataV1>,
}

#[test]
fn package_metadata_v1_decodes_legacy_module_metadata_layout() {
    let package_id = ObjectID::from_single_byte(0xA);
    let legacy_metadata = LegacyPackageMetadataV1 {
        uid: UID::new(package_id),
        storage_id: ID::new(package_id),
        runtime_id: ID::new(package_id),
        package_version: 1,
        modules_metadata: VecMap {
            contents: vec![Entry {
                key: "M".to_owned(),
                value: LegacyModuleMetadataV1 {
                    authenticator_metadata: vec![AuthenticatorMetadataV1 {
                        function_name: "authenticate".to_owned(),
                        account_type: TypeName::from(&TypeTag::Bool),
                    }],
                },
            }],
        },
    };

    let bytes = bcs::to_bytes(&legacy_metadata).unwrap();
    let decoded: PackageMetadataV1 = match bcs::from_bytes(&bytes) {
        Ok(decoded) => decoded,
        Err(error) => {
            eprintln!("failed to decode legacy PackageMetadataV1 bytes: {error}");
            panic!("PackageMetadataV1 must keep decoding existing on-chain metadata objects");
        }
    };

    let module_metadata = &decoded.modules_metadata.contents[0].value;
    assert_eq!(module_metadata.authenticator_metadata.len(), 1);
}
