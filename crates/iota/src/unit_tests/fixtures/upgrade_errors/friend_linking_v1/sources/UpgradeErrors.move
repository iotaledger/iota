// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Module: UpgradeErrors

#[allow(unused_field)]
module upgrades::upgrades {
    fun call_friend() {
        upgrades::upgrades_friend::friend_to_be_dropped();
    }
}

module upgrades::upgrades_friend {
    public(package) fun friend_to_be_dropped() {}
}