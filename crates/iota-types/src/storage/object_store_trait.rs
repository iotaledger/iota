// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{collections::BTreeMap, sync::Arc};

use super::{ObjectKey, error::Result};
use crate::{
    base_types::{ObjectID, ObjectRef, VersionNumber},
    object::Object,
    storage::WriteKind,
};

pub trait ObjectStore {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>>;

    /// Non-fallible version of `try_get_object`.
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.try_get_object(object_id)
            .expect("storage access failed")
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>>;

    /// Non-fallible version of `try_get_object_by_key`.
    fn get_object_by_key(&self, object_id: &ObjectID, version: VersionNumber) -> Option<Object> {
        self.try_get_object_by_key(object_id, version)
            .expect("storage access failed")
    }

    fn try_multi_get_objects(&self, object_ids: &[ObjectID]) -> Result<Vec<Option<Object>>> {
        object_ids
            .iter()
            .map(|digest| self.try_get_object(digest))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Non-fallible version of `try_multi_get_objects`.
    fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        self.try_multi_get_objects(object_ids)
            .expect("storage access failed")
    }

    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Result<Vec<Option<Object>>> {
        object_keys
            .iter()
            .map(|k| self.try_get_object_by_key(&k.0, k.1))
            .collect::<Result<Vec<_>, _>>()
    }

    /// Non-fallible version of `try_multi_get_objects_by_key`.
    fn multi_get_objects_by_key(&self, object_keys: &[ObjectKey]) -> Vec<Option<Object>> {
        self.try_multi_get_objects_by_key(object_keys)
            .expect("storage access failed")
    }
}

impl<T: ObjectStore + ?Sized> ObjectStore for &T {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        (*self).try_get_object(object_id)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        (*self).try_get_object_by_key(object_id, version)
    }

    fn try_multi_get_objects(&self, object_ids: &[ObjectID]) -> Result<Vec<Option<Object>>> {
        (*self).try_multi_get_objects(object_ids)
    }

    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Result<Vec<Option<Object>>> {
        (*self).try_multi_get_objects_by_key(object_keys)
    }
}

impl<T: ObjectStore + ?Sized> ObjectStore for Box<T> {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        (**self).try_get_object(object_id)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        (**self).try_get_object_by_key(object_id, version)
    }

    fn try_multi_get_objects(&self, object_ids: &[ObjectID]) -> Result<Vec<Option<Object>>> {
        (**self).try_multi_get_objects(object_ids)
    }

    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Result<Vec<Option<Object>>> {
        (**self).try_multi_get_objects_by_key(object_keys)
    }
}

impl<T: ObjectStore + ?Sized> ObjectStore for Arc<T> {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        (**self).try_get_object(object_id)
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        (**self).try_get_object_by_key(object_id, version)
    }

    fn try_multi_get_objects(&self, object_ids: &[ObjectID]) -> Result<Vec<Option<Object>>> {
        (**self).try_multi_get_objects(object_ids)
    }

    fn try_multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Result<Vec<Option<Object>>> {
        (**self).try_multi_get_objects_by_key(object_keys)
    }
}

impl ObjectStore for &[Object] {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        Ok(self.iter().find(|o| o.id() == *object_id).cloned())
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        Ok(self
            .iter()
            .find(|o| o.id() == *object_id && o.version() == version)
            .cloned())
    }
}

impl ObjectStore for BTreeMap<ObjectID, (ObjectRef, Object, WriteKind)> {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        Ok(self.get(object_id).map(|(_, obj, _)| obj).cloned())
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        Ok(self
            .get(object_id)
            .and_then(|(_, obj, _)| {
                if obj.version() == version {
                    Some(obj)
                } else {
                    None
                }
            })
            .cloned())
    }
}

impl ObjectStore for BTreeMap<ObjectID, Object> {
    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        Ok(self.get(object_id).cloned())
    }

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>> {
        Ok(self.get(object_id).and_then(|o| {
            if o.version() == version {
                Some(o.clone())
            } else {
                None
            }
        }))
    }
}
