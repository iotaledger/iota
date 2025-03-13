// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// All the vested rewards migrated from Stardust are labeled with this label.
/// It can not be added to an object later after the migration.
module stardust::stardust_upgrade_label;

/// Name of the label.
public struct STARDUST_UPGRADE_LABEL has drop {}
