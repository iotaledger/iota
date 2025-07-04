// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::ed25519::Ed25519KeyPair;
use iota_json_rpc_api::{CoinReadApiClient, GovernanceReadApiClient};
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_swarm_config::genesis_config::{AccountConfig, DEFAULT_GAS_AMOUNT, GenesisConfig};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    crypto::{IotaKeyPair, get_key_pair_from_rng},
    gas_coin::NANOS_PER_IOTA,
};
use test_cluster::TestClusterBuilder;

/// This e2e test ensures that the tokenomics implementation gives an ~8% APY
/// under certain assumptions. These assumptions are:
///
/// - A total stake of 3.5B IOTA.
/// - The default validator commission of 2%.
/// - A validator subsidy (target reward) of 767K IOTA.
///
/// This test uses the TestCluster which has limitations on how the validators
/// can be set up. Only the validator committee size can be changed, but not
/// their initial stakes, so this complicates the test a little. We use the
/// default number of 4 validators and their initial stake of
/// VALIDATOR_LOW_STAKE_THRESHOLD_NANOS. Note that in this case, each validator
/// has 25% of the total voting power which results in each pool getting 25% of
/// the subsidy. In order to get the total stake up to the 3.5B IOTA, we
/// would have to add that amount of stake to *each* pool. But: APY is
/// calculated from the exchange rates of a single pool, which is independent of
/// the total stake. So we actually only need to add a quarter of that stake
/// (875M IOTA) to a single pool. Hence, in the test we delegate that
/// number of IOTAs to a validator.
/// Note that this imbalance doesn't mean this pool has a higher voting power
/// and thus gets more rewards, it still gets 25%. See the voting power
/// calculation function for why that is.
///
/// At least two exchanges rates are needed to calculate the APY in the API.
/// Epoch 0 always has an initial exchange rate set which cannot be used, so we
/// need to calculate APY from later epochs. We use three epochs, to augment the
/// exchange-rate sample and avoid tampering the statistical estimate of the
/// APY. Since we need epoch 0 to start staking anyway, and only have the stake
/// of the pool at the expected number (a quarter of 3.5B IOTAs) starting from
/// epoch 1, this is totally fine.
#[sim_test]
async fn test_apy() {
    // clean up the `cached` cache before running the test.
    #[cfg(msim)]
    {
        iota_json_rpc::governance_api::clear_exchange_rates_cache_for_testing().await;
    }

    // We need a large stake for low enough APY values such that they are not
    // filtered out by the APY calculation function.
    let pool_stake = 3_500_000_000 * NANOS_PER_IOTA / 4;
    let mut rng = rand::thread_rng();
    let mut genesis_config = GenesisConfig::for_local_testing();
    let (address, keypair): (_, Ed25519KeyPair) = get_key_pair_from_rng(&mut rng);
    genesis_config.accounts.extend([AccountConfig {
        address: Some(address),
        gas_amounts: vec![DEFAULT_GAS_AMOUNT, pool_stake],
    }]);

    let mut test_cluster = TestClusterBuilder::new()
        .set_genesis_config(genesis_config)
        .with_epoch_duration_ms(10_000)
        .with_num_validators(4)
        .build()
        .await;

    // We need to add the key to the wallet store since a transaction must be signed
    // for that address.
    test_cluster
        .wallet
        .config_mut()
        .keystore_mut()
        .add_key(None, IotaKeyPair::Ed25519(keypair))
        .unwrap();

    let ref_gas_price = test_cluster.get_reference_gas_price().await;

    let client = test_cluster.rpc_client();
    let mut coins = client
        .get_coins(address, None, None, None)
        .await
        .unwrap()
        .data
        .into_iter();
    let (gas_coin, stake_coin) = {
        let coin1 = coins.next().expect("there should be at least two coins");
        let coin2 = coins.next().expect("there should be at least two coins");

        if coin1.balance > coin2.balance {
            (coin2, coin1)
        } else {
            (coin1, coin2)
        }
    };

    let validator_address = test_cluster
        .swarm
        .active_validators()
        .next()
        .unwrap()
        .config()
        .iota_address();
    let transaction = TestTransactionBuilder::new(address, gas_coin.object_ref(), ref_gas_price)
        .call_staking(stake_coin.object_ref(), validator_address)
        .build();
    test_cluster
        .sign_and_execute_transaction(&transaction)
        .await;

    // Wait for three epochs with the new stake so we get an accurate
    // statistical estimate of the APY.
    test_cluster.wait_for_epoch(None).await;
    test_cluster.wait_for_epoch(None).await;
    test_cluster.wait_for_epoch(None).await;

    let http_client = test_cluster.rpc_client();

    let apys = http_client
        .get_validators_apy()
        .await
        .expect("call should succeed");

    assert_eq!(apys.epoch, 3);

    let validator_apy = apys
        .apys
        .iter()
        .find(|validator_apy| validator_apy.address == validator_address)
        .unwrap();

    // See description above for the origin of this value.
    // Assert that the value is off by at most 0.2 percentage points.
    assert!((validator_apy.apy - 0.08).abs() < 0.002);
}
