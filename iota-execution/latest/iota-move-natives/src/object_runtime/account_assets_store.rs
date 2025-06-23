use std::collections::{BTreeMap, btree_map};

use iota_types::{
    base_types::{MoveObjectType, ObjectID},
    object::{Data, MoveObject, Object, Owner},
    storage::AccountAssetObjectResolver,
};
use move_binary_format::errors::{PartialVMError, PartialVMResult};
use move_core_types::{
    account_address::AccountAddress, annotated_value as A, runtime_value as R,
    vm_status::StatusCode,
};
use move_vm_types::{
    loaded_data::runtime_types::Type,
    values::{GlobalValue, Value},
};

pub(crate) enum AssetResult<V> {
    // Asset exists but type does not match.
    MismatchedType,
    Loaded(V),
}

pub(super) struct AssetObject {
    pub(super) _owner: AccountAddress,
    pub(super) _ty: Type,
    pub(super) move_type: MoveObjectType,
    pub(super) value: GlobalValue,
}

macro_rules! fetch_asset_object_unbounded {
    ($inner:ident, $account:ident, $asset:ident/*, $parents_root_version:expr, $had_parent_root_version:expr */) => {{
        let asset_opt = $inner
            .resolver
            .read_account_asset(&$account.into(), &$asset)
            .map_err(|msg| {
                PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(format!("{msg}"))
            })?;
        if let Some(object) = asset_opt {
            // TODO: Investigate if we need to check object versions.

            // if there was no root version, guard against reading a child object. A newly
            // created parent should not have a child in storage
            // if !$had_parent_root_version {
            //     return Err(
            //         PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(format!(
            //             "A new parent {} should not have a child object {}.",
            //             $parent, $child
            //         )),
            //     );
            // }
            // guard against bugs in `read_child_object`: if it returns a child object such
            // that C.parent != parent, we raise an invariant violation
            match &object.owner {
                Owner::ObjectOwner(_) => {
                    return Err(PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(
                        format!(
                            "Bad owner for {}. \
                            Expected an address owner {} but found an id",
                            $asset, $account
                        ),
                    ));
                }
                Owner::Immutable | Owner::Shared { .. } => {
                    return Err(PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(
                        format!(
                            "Bad owner for {}. \
                            Expected an address owner {} but found immutable or shared owner",
                            $asset, $account
                        ),
                    ));
                }
                Owner::AddressOwner(address) => {
                    if *address != $account.into() {
                        return Err(PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(
                            format!(
                                "Bad owner for {}. Expected owner {} but found owner {}",
                                $asset, $account, address
                            ),
                        ));
                    }

                    // TODO: Ensure that the account address is an Account
                    // Abstraction one.
                }
            };
            match object.data {
                Data::Package(_) => {
                    return Err(PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(
                        format!(
                            "Mismatched object type for {}. \
                            Expected a Move object but found a Move package",
                            $asset
                        ),
                    ));
                }
                Data::Move(_) => Some(object),
            }
        } else {
            None
        }
    }};
}

struct Inner<'a> {
    // used for loading assets
    resolver: &'a dyn AccountAssetObjectResolver,

    cached_objects: BTreeMap<ObjectID, Option<Object>>,
}

// Maintains the runtime GlobalValues for account asset objects and manages the
// fetching of objects from storage, through the `AccountAssetObjectResolver`.
pub(super) struct AccountAssetsObjectStore<'a> {
    // Contains assets resolver and object cache.
    // Kept as a separate struct to deal with lifetime issues where the `store` is accessed at the
    // same time as the `cached_objects` is populated.
    inner: Inner<'a>,
    // Maps of populated GlobalValues, meaning the asset object has been accessed in this
    // transaction.
    store: BTreeMap<ObjectID, AssetObject>,
}

impl<'a> AccountAssetsObjectStore<'a> {
    pub fn new(resolver: &'a dyn AccountAssetObjectResolver) -> Self {
        AccountAssetsObjectStore {
            inner: Inner {
                resolver,
                cached_objects: BTreeMap::new(),
            },
            store: BTreeMap::new(),
        }
    }

    pub(super) fn get_or_fetch_account_asset(
        &mut self,
        account: AccountAddress,
        asset: ObjectID,
        asset_ty: &Type,
        asset_layout: &R::MoveTypeLayout,
        asset_fully_annotated_layout: &A::MoveTypeLayout,
        asset_move_type: MoveObjectType,
    ) -> PartialVMResult<AssetResult<&mut AssetObject>> {
        // let store_entries_count = self.store.len() as u64;
        let asset_object = match self.store.entry(asset) {
            btree_map::Entry::Vacant(e) => {
                let (ty, value) = match self.inner.fetch_object_impl(
                    account,
                    asset,
                    asset_ty,
                    asset_layout,
                    asset_fully_annotated_layout,
                    &asset_move_type,
                )? {
                    AssetResult::MismatchedType => return Ok(AssetResult::MismatchedType),
                    AssetResult::Loaded(res) => res,
                };

                // TODO: Uncomment when the metrics are available.

                // if let LimitThresholdCrossed::Hard(_, lim) = check_limit_by_meter!(
                //     self.is_metered,
                //     store_entries_count,
                //     self.inner
                //         .protocol_config
                //         .object_runtime_max_num_store_entries(),
                //     self.inner
                //         .protocol_config
                //         .object_runtime_max_num_store_entries_system_tx(),
                //     self.inner.metrics.excessive_object_runtime_store_entries
                // ) {
                //     return Err(PartialVMError::new(StatusCode::MEMORY_LIMIT_EXCEEDED)
                //         .with_message(format!(
                //             "Object runtime store limit ({} entries) reached",
                //             lim
                //         ))
                //         .with_sub_status(
                //
                // VMMemoryLimitExceededSubStatusCode::OBJECT_RUNTIME_STORE_LIMIT_EXCEEDED
                //                 as u64,
                //         ));
                // };

                e.insert(AssetObject {
                    _owner: account,
                    _ty: ty,
                    move_type: asset_move_type,
                    value,
                })
            }
            btree_map::Entry::Occupied(e) => {
                let asset_object = e.into_mut();
                if asset_object.move_type != asset_move_type {
                    return Ok(AssetResult::MismatchedType);
                }
                asset_object
            }
        };
        Ok(AssetResult::Loaded(asset_object))
    }
}

impl Inner<'_> {
    fn fetch_object_impl(
        &mut self,
        account: AccountAddress,
        asset: ObjectID,
        asset_ty: &Type,
        asset_layout: &R::MoveTypeLayout,
        _asset_fully_annotated_layout: &A::MoveTypeLayout,
        asset_move_type: &MoveObjectType,
    ) -> PartialVMResult<AssetResult<(Type, GlobalValue)>> {
        let obj = match self.get_or_fetch_object_from_store(account, asset)? {
            None => {
                return Ok(AssetResult::Loaded((asset_ty.clone(), GlobalValue::none())));
            }
            Some(obj) => obj,
        };
        // object exists, but the type does not match
        if obj.type_() != asset_move_type {
            return Ok(AssetResult::MismatchedType);
        }
        // generate a GlobalValue
        let obj_contents = obj.contents();
        let v = match Value::simple_deserialize(obj_contents, asset_layout) {
            Some(v) => v,
            None => return Err(
                PartialVMError::new(StatusCode::FAILED_TO_DESERIALIZE_RESOURCE).with_message(
                    format!("Failed to deserialize object {asset} with type {asset_move_type}",),
                ),
            ),
        };
        let global_value =
            match GlobalValue::cached(v) {
                Ok(gv) => gv,
                Err(e) => {
                    return Err(PartialVMError::new(StatusCode::STORAGE_ERROR).with_message(
                        format!("Object {asset} did not deserialize to a struct Value. Error: {e}"),
                    ));
                }
            };

        // TODO: Investigate if we need to check object versions.

        // // Find all UIDs inside of the value and update the object parent maps
        // let contained_uids =
        //     get_all_uids(asset_fully_annotated_layout, obj_contents).map_err(|e| {
        //         PartialVMError::new(StatusCode::UNKNOWN_INVARIANT_VIOLATION_ERROR)
        //             .with_message(format!("Failed to find UIDs. ERROR: {e}"))
        //     })?;
        // let parents_root_version = self.root_version.get(&parent).copied();
        // if let Some(v) = parents_root_version {
        //     debug_assert!(contained_uids.contains(&child));
        //     for id in contained_uids {
        //         self.root_version.insert(id, v);
        //         if id != child {
        //             let prev = self.wrapped_object_containers.insert(id, child);
        //             debug_assert!(prev.is_none())
        //         }
        //     }
        // }

        Ok(AssetResult::Loaded((asset_ty.clone(), global_value)))
    }

    fn get_or_fetch_object_from_store(
        &mut self,
        account: AccountAddress,
        asset: ObjectID,
    ) -> PartialVMResult<Option<&MoveObject>> {
        // TODO: Investigate if we need to check object versions.

        // let cached_objects_count = self.cached_objects.len() as u64;
        // let parents_root_version = self.root_version.get(&parent).copied();
        // let had_parent_root_version = parents_root_version.is_some();
        // // if not found, it must be new so it won't have any child objects, thus
        // // we can return SequenceNumber(0) as no child object will be found
        // let parents_root_version =
        // parents_root_version.unwrap_or(SequenceNumber::new());
        if let btree_map::Entry::Vacant(e) = self.cached_objects.entry(asset) {
            let obj_opt = fetch_asset_object_unbounded!(
                self, account,
                asset /* parents_root_version,
                       * had_parent_root_version */
            );

            // TODO: Uncomment when the metrics are available.

            // if let LimitThresholdCrossed::Hard(_, lim) = check_limit_by_meter!(
            //     self.is_metered,
            //     cached_objects_count,
            //     self.protocol_config.object_runtime_max_num_cached_objects(),
            //     self.protocol_config
            //         .object_runtime_max_num_cached_objects_system_tx(),
            //     self.metrics.excessive_object_runtime_cached_objects
            // ) {
            //     return Err(PartialVMError::new(StatusCode::MEMORY_LIMIT_EXCEEDED)
            //         .with_message(format!(
            //             "Object runtime cached objects limit ({} entries) reached",
            //             lim
            //         ))
            //         .with_sub_status(
            //
            // VMMemoryLimitExceededSubStatusCode::OBJECT_RUNTIME_CACHE_LIMIT_EXCEEDED
            //                 as u64,
            //         ));
            // };

            e.insert(obj_opt);
        }
        Ok(self
            .cached_objects
            .get(&asset)
            .unwrap()
            .as_ref()
            .map(|obj| {
                obj.data
                    .try_as_move()
                    // unwrap safe because we only insert Move objects
                    .unwrap()
            }))
    }
}
