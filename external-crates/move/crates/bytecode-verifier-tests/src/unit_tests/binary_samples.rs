// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Tests in here are based on binary representation of modules taken from
//! production. Those tests may fail over time if the representation becomes out
//! of date, then they can be removed. Right now the serve to calibrate the
//! metering working as expected. Those tests represent cases which we want to
//! continue to succeed.

use move_binary_format::{CompiledModule, errors::VMResult};
use move_bytecode_verifier::verifier;
use move_bytecode_verifier_meter::bound::BoundMeter;

use crate::unit_tests::production_config;

#[allow(unused)]
fn run_binary_test(name: &str, bytes: &str) -> VMResult<()> {
    let bytes = hex::decode(bytes).expect("invalid hex string");
    let m = CompiledModule::deserialize_with_defaults(&bytes).expect("invalid module");
    let (verifier_config, meter_config) = production_config();
    let mut meter = BoundMeter::new(meter_config);
    verifier::verify_module_with_config_for_test(name, &verifier_config, &m, &mut meter)
}
