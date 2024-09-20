// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_types::object::Owner;
use move_core_types::language_storage::StructTag;

use crate::client::response::{Created, Deleted, Mutated, Transferred, Wrapped};

/// Unifier for transaction result objects.
pub trait TxnObject {
    /// The type of the object.
    fn ty(&self) -> &StructTag;

    /// The type name of the object.
    fn name(&self) -> &str {
        self.ty().name.as_str()
    }

    /// The owner of the object.
    fn owner(&self) -> Option<&Owner> {
        None
    }
}

impl TxnObject for Created {
    fn ty(&self) -> &StructTag {
        &self.object_type
    }

    fn owner(&self) -> Option<&Owner> {
        Some(&self.owner)
    }
}

impl TxnObject for Wrapped {
    fn ty(&self) -> &StructTag {
        &self.object_type
    }
}

impl TxnObject for Deleted {
    fn ty(&self) -> &StructTag {
        &self.object_type
    }
}

impl TxnObject for Mutated {
    fn ty(&self) -> &StructTag {
        &self.object_type
    }

    fn owner(&self) -> Option<&Owner> {
        Some(&self.owner)
    }
}

impl TxnObject for Transferred {
    fn ty(&self) -> &StructTag {
        &self.object_type
    }

    fn owner(&self) -> Option<&Owner> {
        Some(&self.recipient)
    }
}
