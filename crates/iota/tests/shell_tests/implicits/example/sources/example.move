// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module example::example;

use iota::kiosk::{new as new_kiosk, Kiosk, KioskOwnerCap};

public fun create_new_kiosk(ctx: &mut TxContext): (Kiosk, KioskOwnerCap) {
  new_kiosk(ctx)
}
