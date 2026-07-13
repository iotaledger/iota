// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use fastcrypto::ed25519::Ed25519KeyPair;
use iota_grpc_client::ReadMask;
use iota_json_rpc::governance_api::{ValidatorExchangeRates, calculate_apys};
use iota_json_rpc_types::ValidatorApys;
use iota_keys::keystore::AccountKeystore;
use iota_macros::sim_test;
use iota_sdk_types::ObjectReference;
use iota_swarm_config::genesis_config::{AccountConfig, DEFAULT_GAS_AMOUNT, GenesisConfig};
use iota_test_transaction_builder::TestTransactionBuilder;
use iota_types::{
    committee::EpochId,
    crypto::{IotaKeyPair, get_key_pair_from_rng},
    gas_coin::{GasCoin, NANOS_PER_IOTA},
    iota_system_state::{
        PoolTokenExchangeRate, iota_system_state_summary::IotaSystemStateSummaryV2,
    },
};
use test_cluster::{TestCluster, TestClusterBuilder};

/// This e2e test ensures that the tokenomics implementation gives an ~6% APY
/// under certain assumptions. These assumptions are:
///
/// - A total stake of 3.5B IOTA.
/// - The default validator commission of 2%.
/// - A validator subsidy (target reward) of 767K IOTA.
///
/// Note: Without IIP-8, the expected APY would be ~8%. However, with 4
/// validators each having 25% voting power (2500 bp), the IIP-8 dynamic
/// minimum commission of `max(commission_rate, voting_power)` results in an
/// effective commission of 25% instead of 2%. This reduces the staker APY
/// to ~6%.
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
        .with_fullnode_enable_grpc_api(true)
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

    // The address owns exactly its two genesis coins. Read them from node state
    // (the gRPC owned-object index is checkpoint-derived and may not yet contain
    // genesis objects this early) and pick the smaller (gas) and larger (stake)
    // coin.
    let mut coins: Vec<(u64, ObjectReference)> = test_cluster
        .fullnode_handle
        .iota_node
        .with_async(|node| async move {
            let infos = node
                .state()
                .get_owner_objects(address, None, 10, None)
                .expect("owned-object lookup should succeed");
            let mut coins = Vec::new();
            for info in infos {
                let object = node
                    .state()
                    .get_object(&info.object_id)
                    .expect("owned object should exist");
                let value = GasCoin::try_from(&object)
                    .expect("owned object should be an IOTA coin")
                    .value();
                coins.push((value, object.object_ref()));
            }
            coins
        })
        .await;
    assert_eq!(coins.len(), 2, "expected exactly the two genesis coins");
    coins.sort_by_key(|(value, _)| *value);
    let (gas_coin_ref, stake_coin_ref) = (coins[0].1, coins[1].1);

    let validator_address = test_cluster
        .swarm
        .active_validators()
        .next()
        .unwrap()
        .config()
        .iota_address();
    let transaction = TestTransactionBuilder::new(address, gas_coin_ref, ref_gas_price)
        .call_staking(stake_coin_ref, validator_address)
        .build();
    test_cluster
        .sign_and_execute_transaction(&transaction)
        .await;

    // Wait for three epochs with the new stake so we get an accurate
    // statistical estimate of the APY.
    test_cluster.wait_for_epoch(None).await;
    test_cluster.wait_for_epoch(None).await;
    test_cluster.wait_for_epoch(None).await;

    let apys = grpc_validators_apy(&test_cluster).await;

    assert_eq!(apys.epoch, 3);

    let validator_apy = apys
        .apys
        .iter()
        .find(|validator_apy| validator_apy.address == validator_address)
        .unwrap();

    // See description above for the origin of this value.
    // With IIP-8, effective commission = max(2%, 25%) = 25% for 4 validators.
    // APY = deposit_per_epoch / pool_balance * 365
    //     = 191750 * 0.75 / 876_500_000 * 365 ≈ 0.060.
    // Assert that the value is off by at most 0.2 percentage points.
    assert!((validator_apy.apy - 0.06).abs() < 0.002);
}

/// Replicate `get_validators_apy` over the node gRPC API: read the current
/// system state via GetEpoch, walk each active validator's exchange-rate table
/// via `list_dynamic_fields` (name = epoch, value = `PoolTokenExchangeRate`),
/// then reuse the node's `calculate_apys`.
async fn grpc_validators_apy(test_cluster: &TestCluster) -> ValidatorApys {
    let client = test_cluster.grpc_client();

    let summary =
        IotaSystemStateSummaryV2::try_from(test_cluster.grpc_system_state_summary().await).unwrap();
    let epoch = summary.epoch;

    let mut exchange_rate_table = Vec::new();
    for validator in summary.active_validators {
        let fields = client
            .list_dynamic_fields(
                validator.exchange_rates_id,
                None,
                None,
                Some(ReadMask::from(&["name", "value"][..])),
            )
            .collect(None)
            .await
            .unwrap();

        let mut rates: Vec<(EpochId, PoolTokenExchangeRate)> = fields
            .body()
            .iter()
            .map(|df| {
                let rate_epoch: EpochId = df.name.as_ref().unwrap().deserialize().unwrap();
                let rate: PoolTokenExchangeRate = df.value.as_ref().unwrap().deserialize().unwrap();
                (rate_epoch, rate)
            })
            .collect();
        // `calculate_apys` expects rates in descending epoch order (as produced by
        // the node's `backfill_rates`).
        rates.sort_unstable_by_key(|b| std::cmp::Reverse(b.0));

        exchange_rate_table.push(ValidatorExchangeRates {
            address: validator.iota_address,
            pool_id: validator.staking_pool_id,
            active: true,
            rates,
        });
    }

    ValidatorApys {
        apys: calculate_apys(exchange_rate_table),
        epoch,
    }
}
