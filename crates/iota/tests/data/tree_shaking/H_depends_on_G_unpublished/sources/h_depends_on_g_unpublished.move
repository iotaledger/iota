// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module h_depends_on_g_unpublished::h_depends_on_g_unpublished {
    public fun h() {
        g_unpublished::g_unpublished::g();
    }
}

