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

pub trait ObjectStoreFallible {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.try_get_object(object_id).unwrap()
    }

    fn get_object_by_key(&self, object_id: &ObjectID, version: VersionNumber) -> Option<Object> {
        self.try_get_object_by_key(object_id, version).unwrap()
    }

    fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        self.try_multi_get_objects(object_ids).unwrap()
    }

    fn multi_get_objects_by_key(&self, object_keys: &[ObjectKey]) -> Vec<Option<Object>> {
        self.try_multi_get_objects_by_key(object_keys).unwrap()
    }

    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>>;

    fn try_get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Result<Option<Object>>;

    fn try_multi_get_objects(&self, object_ids: &[ObjectID]) -> Result<Vec<Option<Object>>> {
        object_ids
            .iter()
            .map(|digest| self.try_get_object(digest))
            .collect::<Result<Vec<_>, _>>()
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
}

impl<T: ObjectStoreFallible + ?Sized> ObjectStoreFallible for &T {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        (*self).get_object(object_id)
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Option<Object> {
        (*self).get_object_by_key(object_id, version)
    }

    fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        (*self).multi_get_objects(object_ids)
    }

    fn multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Vec<Option<Object>> {
        (*self).multi_get_objects_by_key(object_keys)
    }

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

impl<T: ObjectStoreFallible + ?Sized> ObjectStoreFallible for Box<T> {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        (**self).get_object(object_id)
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Option<Object> {
        (**self).get_object_by_key(object_id, version)
    }

    fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        (**self).multi_get_objects(object_ids)
    }

    fn multi_get_objects_by_key(
        &self,
        object_keys: &[ObjectKey],
    ) -> Vec<Option<Object>> {
        (**self).multi_get_objects_by_key(object_keys)
    }

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

impl<T: ObjectStoreFallible + ?Sized> ObjectStoreFallible for Arc<T> {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        (**self).get_object(object_id)
    }

    fn get_object_by_key(&self, object_id: &ObjectID, version: VersionNumber) -> Option<Object> {
        (**self).get_object_by_key(object_id, version)
    }

    fn multi_get_objects(&self, object_ids: &[ObjectID]) -> Vec<Option<Object>> {
        (**self).multi_get_objects(object_ids)
    }

    fn multi_get_objects_by_key(&self, object_keys: &[ObjectKey]) -> Vec<Option<Object>> {
        (**self).multi_get_objects_by_key(object_keys)
    }

    fn try_get_object(&self, object_id: &ObjectID) -> Result<Option<Object>> {
        (**self).try_get_object(object_id)
    }

    fn try_get_object_by_key(&self, object_id: &ObjectID, version: VersionNumber) -> Result<Option<Object>> {
        (**self).try_get_object_by_key(object_id, version)
    }

    fn try_multi_get_objects(&self, object_ids: &[ObjectID]) -> Result<Vec<Option<Object>>> {
        (**self).try_multi_get_objects(object_ids)
    }

    fn try_multi_get_objects_by_key(&self, object_keys: &[ObjectKey]) -> Result<Vec<Option<Object>>> {
        (**self).try_multi_get_objects_by_key(object_keys)
    }
}

impl ObjectStoreFallible for &[Object] {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.iter().find(|o| o.id() == *object_id).cloned()
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Option<Object> {
        self
            .iter()
            .find(|o| o.id() == *object_id && o.version() == version)
            .cloned()
    }

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

impl ObjectStoreFallible for BTreeMap<ObjectID, (ObjectRef, Object, WriteKind)> {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.get(object_id).map(|(_, obj, _)| obj).cloned()
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Option<Object> {
        self
            .get(object_id)
            .and_then(|(_, obj, _)| {
                if obj.version() == version {
                    Some(obj)
                } else {
                    None
                }
            })
            .cloned()
    }

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

impl ObjectStoreFallible for BTreeMap<ObjectID, Object> {
    fn get_object(&self, object_id: &ObjectID) -> Option<Object> {
        self.get(object_id).cloned()
    }

    fn get_object_by_key(
        &self,
        object_id: &ObjectID,
        version: VersionNumber,
    ) -> Option<Object> {
        self.get(object_id).and_then(|o| {
            if o.version() == version {
                Some(o.clone())
            } else {
                None
            }
        })
    }

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
