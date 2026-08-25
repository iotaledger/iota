// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use iota_protocol_config::ProtocolConfig;
use iota_sdk_types::Address;

use crate::{
    error::UserInputError,
    gas::*,
    object::Object,
    transaction::{InputObjectKind, ObjectReadResult},
};

/// The same gas coins and an under-minimum budget are rejected when the
/// budget is bounded and accepted when it is not, which is the whole of
/// what a simulation relaxes about gas.
#[test]
fn check_gas_balance_bounds_the_budget_only_when_asked() {
    let config = ProtocolConfig::get_for_max_version_UNSAFE();
    let gas_price = config.max_gas_price();
    let gas_object =
        Object::new_gas_with_balance_and_owner_for_testing(1_000_000_000, Address::random());
    let read = ObjectReadResult::new(
        InputObjectKind::ImmOrOwnedMoveObject(gas_object.object_ref()),
        gas_object.into(),
    );
    let gas_objs = [&read];

    let status = IotaGasStatus::new(0, gas_price, gas_price, &config)
        .expect("gas price equal to the reference price is in bounds");

    assert!(matches!(
        status.check_gas_balance(&gas_objs, 0, true),
        Err(UserInputError::GasBudgetTooLow { .. })
    ));
    assert!(status.check_gas_balance(&gas_objs, 0, false).is_ok());
}
