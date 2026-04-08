// Copyright (c) The Diem Core Contributors
// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use move_binary_format::file_format::CompiledModule;
use proptest::prelude::*;

proptest! {
    #[test]
    fn serializer_roundtrip(module in CompiledModule::valid_strategy(20)) {
        let mut serialized = Vec::with_capacity(2048);
        module.serialize(&mut serialized).expect("serialization should work");

        let deserialized_module = CompiledModule::deserialize_with_defaults(&serialized)
            .expect("deserialization should work");

        prop_assert_eq!(module, deserialized_module);
    }
}

/// Make sure that garbage inputs don't crash the serializer and deserializer.
///
/// Runs on a thread with an 8 MiB stack because deeply-nested `SignatureToken`
/// generation in proptest can overflow the default 2 MiB test-thread stack in
/// debug builds.
#[test]
fn garbage_inputs() {
    const STACK_SIZE: usize = 8 * 1024 * 1024;
    let child = std::thread::Builder::new()
        .stack_size(STACK_SIZE)
        .spawn(|| {
            proptest!(ProptestConfig::with_cases(16), |(module in any_with::<CompiledModule>(16))| {
                let mut serialized = Vec::with_capacity(65536);
                module.serialize(&mut serialized).expect("serialization should work");

                let deserialized_module = CompiledModule::deserialize_no_check_bounds(&serialized)
                    .expect("deserialization should work");
                prop_assert_eq!(module, deserialized_module);
            });
        })
        .expect("failed to spawn thread");
    child.join().expect("garbage_inputs thread panicked");
}
