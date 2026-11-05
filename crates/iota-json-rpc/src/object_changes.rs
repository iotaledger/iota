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
        .map(|modified| (*modified.object_id(), modified.version()))
        .collect::<BTreeMap<_, _>>();

    for (changed, kind) in all_changed_objects {
        let (reference, owner) = (*changed.reference(), *changed.owner());
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
                _ => {
                    unimplemented!("a new WriteKind enum variant was added and needs to be handled")
                }
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
        // The version the transaction consumed is the last one below the
        // removal. Reading it by its exact version lets a provider answer from
        // the input objects it was given, instead of from a store that may have
        // pruned it since.
        let input_object = match modify_at_version.get(&id) {
            Some(input_version) => object_provider.get_object(&id, input_version).await.ok(),
            None => None,
        };
        let o = match input_object {
            Some(o) => Some(o),
            None => {
                object_provider
                    .find_object_lt_or_eq_version(&id, &version)
                    .await?
            }
        };
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
                    _ => unimplemented!(
                        "a new ObjectRemoveKind enum variant was added and needs to be handled"
                    ),
                }
            }
        };
    }

    Ok(object_changes)
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use iota_sdk_types::{ObjectDigest, ObjectId, Owner, Version};
    use iota_types::object::Object;

    use super::*;

    /// Serves only the exact versions it was given: every version-bounded
    /// lookup misses, as in a store that has pruned the older versions.
    struct ExactVersions(BTreeMap<(ObjectId, Version), Object>);

    #[async_trait]
    impl ObjectProvider for ExactVersions {
        type Error = String;

        async fn get_object(&self, id: &ObjectId, version: &Version) -> Result<Object, String> {
            self.0
                .get(&(*id, *version))
                .cloned()
                .ok_or_else(|| format!("{id} at {version} not found"))
        }

        async fn find_object_lt_or_eq_version(
            &self,
            _id: &ObjectId,
            _version: &Version,
        ) -> Result<Option<Object>, String> {
            Ok(None)
        }
    }

    #[tokio::test]
    async fn a_removed_object_is_described_by_the_version_it_was_removed_at() {
        let sender = Address::ZERO;
        let input_version = Version::from(3);
        let removed_at = Version::from(7);
        let (deleted, wrapped) = (ObjectId::random(), ObjectId::random());
        let provider = ExactVersions(
            [deleted, wrapped]
                .into_iter()
                .map(|id| {
                    let object = Object::with_id_owner_version_for_testing(
                        id,
                        input_version,
                        Owner::Address(sender),
                    );
                    ((id, input_version), object)
                })
                .collect(),
        );
        let removed = |id| ObjectReference {
            object_id: id,
            version: removed_at,
            digest: ObjectDigest::new([0; 32]),
        };

        let changes = get_object_changes(
            &provider,
            sender,
            vec![
                ObjectVersion::new(deleted, input_version),
                ObjectVersion::new(wrapped, input_version),
            ],
            vec![],
            vec![
                (removed(deleted), ObjectRemoveKind::Delete),
                (removed(wrapped), ObjectRemoveKind::Wrap),
            ],
        )
        .await
        .unwrap();

        assert!(
            matches!(
                changes.as_slice(),
                [
                    ObjectChange::Deleted { object_id: d, version: dv, .. },
                    ObjectChange::Wrapped { object_id: w, version: wv, .. },
                ] if *d == deleted && *dv == removed_at && *w == wrapped && *wv == removed_at
            ),
            "{changes:?}"
        );
    }
}
