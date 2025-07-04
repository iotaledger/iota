// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_core_types::{ident_str, identifier::IdentStr};

pub const DETERMINISTIC_OBJECT_MODULE_NAME: &IdentStr = ident_str!("deterministic_object_id");
pub const DETERMINISTIC_OBJECT_PRE_COMPUTED_FUNCTION_NAME: &IdentStr =
    ident_str!("new_precomputed");
