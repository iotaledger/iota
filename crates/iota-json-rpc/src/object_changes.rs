// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use iota_json_rpc_types::ObjectChange;
use iota_sdk_types::{
    Address, ObjectReference, ObjectRemoveKind, ObjectVersion, OwnedObjectReference, StructTag,
    WriteKind,
};

use crate::ObjectProvider;

pub async fn get_object_changes<P: ObjectProvider<Error = E>, E>(
    object_provider: &P,
    sender: Address,
    modified_at_versions: Vec<ObjectVersion>,
    all_changed_objects: Vec<(OwnedObjectReference, WriteKind)>,
    all_removed_objects: Vec<(ObjectReference, ObjectRemoveKind)>,
) -> Result<Vec<ObjectChange>, E> {
    let mut object_changes = vec![];

    let modify_at_version = modified_at_versions
        .into_iter()
        .map(|modified| (modified.object_id, modified.version))
        .collect::<BTreeMap<_, _>>();

    for (changed, kind) in all_changed_objects {
        let OwnedObjectReference { reference, owner } = changed;
        let ObjectReference {
            object_id,
            version,
            digest,
        } = reference;
        let o = object_provider.get_object(&object_id, &version).await?;
        if let Some(object_type) = o.data.opt_object_type() {
            let object_type: StructTag = object_type.clone().into();

            match kind {
                WriteKind::Mutate => object_changes.push(ObjectChange::Mutated {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    // modify_at_version should always be available for mutated object
                    previous_version: modify_at_version
                        .get(&object_id)
                        .cloned()
                        .unwrap_or_default(),
                    digest,
                }),
                WriteKind::Create => object_changes.push(ObjectChange::Created {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                }),
                WriteKind::Unwrap => object_changes.push(ObjectChange::Unwrapped {
                    sender,
                    owner,
                    object_type,
                    object_id,
                    version,
                    digest,
                }),
            }
        } else if let Some(p) = o.data.as_opt_package() {
            if kind == WriteKind::Create {
                object_changes.push(ObjectChange::Published {
                    package_id: p.id(),
                    version: p.version(),
                    digest,
                    modules: p
                        .serialized_module_map()
                        .keys()
                        .map(|k| k.to_string())
                        .collect(),
                })
            }
        };
    }

    for (removed_object, kind) in all_removed_objects {
        let id = removed_object.object_id;
        let version = removed_object.version;
        let o = object_provider
            .find_object_lt_or_eq_version(&id, &version)
            .await?;
        if let Some(o) = o {
            if let Some(object_type) = o.data.opt_object_type() {
                let object_type: StructTag = object_type.clone().into();
                match kind {
                    ObjectRemoveKind::Delete => object_changes.push(ObjectChange::Deleted {
                        sender,
                        object_type,
                        object_id: id,
                        version,
                    }),
                    ObjectRemoveKind::Wrap => object_changes.push(ObjectChange::Wrapped {
                        sender,
                        object_type,
                        object_id: id,
                        version,
                    }),
                }
            }
        };
    }

    Ok(object_changes)
}
