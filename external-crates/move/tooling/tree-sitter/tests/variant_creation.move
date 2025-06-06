// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module Completion::test {
    public enum SomeEnum has drop {
        SomeVariant,
    }

    public fun test() {
        let _local = Completion::test::SomeEnum::SomeVariant;
        let _other_local = Self::SomeEnum::SomeVariant;
        let _other_other_local = SomeEnum::SomeVariant;
    }
}
