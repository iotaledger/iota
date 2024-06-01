use iota_sdk::{
    types::block::output::{feature::Irc30Metadata, AliasId, SimpleTokenScheme},
    U256,
};
use sui_types::gas_coin::GAS;

use crate::stardust::migration::tests::{create_foundry, run_migration};

#[test]
fn create_foundry_amount() {
    let (foundry_header, foundry_output) = create_foundry(
        1_000_000,
        SimpleTokenScheme::new(U256::from(100_000), U256::from(0), U256::from(100_000_000))
            .unwrap(),
        Irc30Metadata::new("Rustcoin", "Rust", 0),
        AliasId::null(),
    );

    let objects = run_migration([(foundry_header, foundry_output.into())]).into_objects();

    // Foundry package publication creates five objects
    //
    // * The package
    // * Coin metadata
    // * MaxSupplyPolicy
    // * The total supply coin
    // * The foundry amount coin
    assert_eq!(objects.len(), 5);

    let gas_coin_object = objects
        .into_iter()
        .find_map(|object| object.is_gas_coin().then_some(object))
        .expect("there should be only a single gas coin");
    let coin = gas_coin_object.as_coin_maybe().unwrap();

    assert_eq!(coin.value(), 1_000_000);
    assert_eq!(gas_coin_object.coin_type_maybe().unwrap(), GAS::type_tag());
}
