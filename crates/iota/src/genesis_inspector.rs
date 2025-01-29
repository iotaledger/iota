// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::collections::BTreeMap;

use inquire::Select;
use iota_config::genesis::UnsignedGenesis;
use iota_types::{
    balance::Balance,
    base_types::ObjectID,
    coin::CoinMetadata,
    gas_coin::{GasCoin, IotaTreasuryCap, NANOS_PER_IOTA},
    governance::StakedIota,
    iota_system_state::IotaValidatorGenesis,
    move_package::MovePackage,
    object::{MoveObject, Object, Owner},
    stardust::output::{AliasOutput, BasicOutput, NftOutput},
    timelock::{
        timelock::{TimeLock, is_timelocked_gas_balance},
        timelocked_staked_iota::TimelockedStakedIota,
    },
};

const STR_ALL: &str = "All";
const STR_EXIT: &str = "Exit";
const STR_IOTA: &str = "Iota";
const STR_STAKED_IOTA: &str = "StakedIota";
const STR_TIMELOCKED_IOTA: &str = "TimelockedIota";
const STR_TIMELOCKED_STAKED_IOTA: &str = "TimelockedStakedIota";
const STR_ALIAS_OUTPUT_IOTA: &str = "AliasOutputIota";
const STR_BASIC_OUTPUT_IOTA: &str = "BasicOutputIota";
const STR_NFT_OUTPUT_IOTA: &str = "NftOutputIota";
const STR_PACKAGE: &str = "Package";
const STR_COIN_METADATA: &str = "CoinMetadata";
const STR_OTHER: &str = "Other";
const STR_IOTA_DISTRIBUTION: &str = "IOTA Distribution";
const STR_OBJECTS: &str = "Objects";
const STR_VALIDATORS: &str = "Validators";

pub(crate) fn examine_genesis_checkpoint(
    genesis: UnsignedGenesis,
    migration_objects: impl Iterator<Item = Object>,
) {
    let system_object = genesis
        .iota_system_object()
        .into_genesis_version_for_tooling();

    // Prepare Validator info
    let validator_set = &system_object.validators.active_validators;
    let validator_map = validator_set
        .iter()
        .map(|v| (v.verified_metadata().name.as_str(), v))
        .collect::<BTreeMap<_, _>>();
    let validator_pool_id_map = validator_set
        .iter()
        .map(|v| (v.staking_pool.id, v))
        .collect::<BTreeMap<_, _>>();

    let mut validator_options: Vec<_> = validator_map.keys().copied().collect();
    validator_options.extend_from_slice(&[STR_ALL, STR_EXIT]);
    println!("Total Number of Validators: {}", validator_set.len());

    // Prepare IOTA distribution info
    let mut iota_distribution = BTreeMap::new();
    let entry = iota_distribution
        .entry("IOTA System".to_string())
        .or_insert_with(BTreeMap::new);
    entry.insert(
        "Storage Fund".to_string(),
        (
            STR_IOTA,
            system_object.storage_fund.non_refundable_balance.value(),
        ),
    );

    // Prepare Object Info
    let mut owner_map = BTreeMap::new();
    let mut package_map = BTreeMap::new();
    let mut iota_map = BTreeMap::new();
    let mut staked_iota_map = BTreeMap::new();
    let mut timelocked_iota_map = BTreeMap::new();
    let mut timelocked_staked_iota_map = BTreeMap::new();
    let mut alias_output_iota_map = BTreeMap::new();
    let mut basic_output_iota_map = BTreeMap::new();
    let mut nft_output_iota_map = BTreeMap::new();
    let mut coin_metadata_map = BTreeMap::new();
    let mut other_object_map = BTreeMap::new();

    let additional_objects = genesis.objects();
    let tot_additional_objects = additional_objects.len();
    let mut total_objects = 0;
    for object in additional_objects.iter().cloned().chain(migration_objects) {
        total_objects += 1;
        let object_id = object.id();
        let object_id_str = object_id.to_string();
        assert_eq!(object.storage_rebate, 0);
        owner_map.insert(object.id(), object.owner);

        match &object.data {
            iota_types::object::Data::Move(move_object) => {
                if let Ok(gas) = GasCoin::try_from(&object) {
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(object_id_str.clone(), (STR_IOTA, gas.value()));
                    iota_map.insert(object.id(), gas);
                } else if let Ok(coin_metadata) = CoinMetadata::try_from(&object) {
                    coin_metadata_map.insert(object.id(), coin_metadata);
                } else if let Ok(staked_iota) = StakedIota::try_from(&object) {
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(object_id_str, (STR_STAKED_IOTA, staked_iota.principal()));
                    // Assert pool id is associated with a known validator.
                    let validator = validator_pool_id_map.get(&staked_iota.pool_id()).unwrap();
                    assert_eq!(validator.staking_pool.id, staked_iota.pool_id());
                    staked_iota_map.insert(object.id(), staked_iota);
                } else if let Ok(timelock_balance) = TimeLock::<Balance>::try_from(&object) {
                    assert!(is_timelocked_gas_balance(
                        &object.struct_tag().expect("should not be a package")
                    ));
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(
                        object_id_str,
                        (STR_TIMELOCKED_IOTA, timelock_balance.locked().value()),
                    );
                    timelocked_iota_map.insert(object.id(), timelock_balance);
                } else if let Ok(alias_output) = AliasOutput::try_from(&object) {
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(
                        object_id_str,
                        (STR_ALIAS_OUTPUT_IOTA, alias_output.balance.value()),
                    );
                    alias_output_iota_map.insert(object.id(), alias_output);
                } else if let Ok(basic_output) = BasicOutput::try_from(&object) {
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(
                        object_id_str,
                        (STR_BASIC_OUTPUT_IOTA, basic_output.balance.value()),
                    );
                    basic_output_iota_map.insert(object.id(), basic_output);
                } else if let Ok(nft_output) = NftOutput::try_from(&object) {
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(
                        object_id_str,
                        (STR_NFT_OUTPUT_IOTA, nft_output.balance.value()),
                    );
                    nft_output_iota_map.insert(object.id(), nft_output);
                } else if let Ok(timelocked_staked_iota) = TimelockedStakedIota::try_from(&object) {
                    let entry = iota_distribution
                        .entry(object.owner.to_string())
                        .or_default();
                    entry.insert(
                        object_id_str,
                        (
                            STR_TIMELOCKED_STAKED_IOTA,
                            timelocked_staked_iota.principal(),
                        ),
                    );
                    // Assert pool id is associated with a known validator.
                    let validator = validator_pool_id_map
                        .get(&timelocked_staked_iota.pool_id())
                        .unwrap();
                    assert_eq!(validator.staking_pool.id, timelocked_staked_iota.pool_id());
                    timelocked_staked_iota_map.insert(object.id(), timelocked_staked_iota);
                } else {
                    other_object_map.insert(object.id(), move_object.clone());
                }
            }
            iota_types::object::Data::Package(p) => {
                package_map.insert(object.id(), p.clone());
            }
        }
    }

    println!(
        "Total Number of Migration Objects: {}",
        total_objects - tot_additional_objects
    );
    println!("Total Number of Objects/Packages: {}", total_objects);

    // Always check the Total Supply
    examine_total_supply(&system_object.iota_treasury_cap, &iota_distribution, false);

    // Main loop for inspection
    let main_options: Vec<&str> =
        vec![STR_IOTA_DISTRIBUTION, STR_VALIDATORS, STR_OBJECTS, STR_EXIT];
    loop {
        let ans = Select::new(
            "Select one main category to examine ('Exit' to exit the program):",
            main_options.clone(),
        )
        .prompt();
        match ans {
            Ok(name) if name == STR_IOTA_DISTRIBUTION => {
                examine_total_supply(&system_object.iota_treasury_cap, &iota_distribution, true)
            }
            Ok(name) if name == STR_VALIDATORS => {
                examine_validators(&validator_options, &validator_map);
            }
            Ok(name) if name == STR_OBJECTS => {
                println!("Examine Objects (total: {})", total_objects);
                examine_object(
                    &owner_map,
                    &validator_pool_id_map,
                    &package_map,
                    &iota_map,
                    &staked_iota_map,
                    &timelocked_iota_map,
                    &timelocked_staked_iota_map,
                    &alias_output_iota_map,
                    &basic_output_iota_map,
                    &nft_output_iota_map,
                    &coin_metadata_map,
                    &other_object_map,
                );
            }
            Ok(name) if name == STR_EXIT => break,
            Ok(_) => (),
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        }
    }
}

#[expect(clippy::ptr_arg)]
fn examine_validators(
    validator_options: &Vec<&str>,
    validator_map: &BTreeMap<&str, &IotaValidatorGenesis>,
) {
    loop {
        let ans = Select::new("Select one validator to examine ('All' to display all Validators, 'Exit' to return to Main):", validator_options.clone()).prompt();
        match ans {
            Ok(name) if name == STR_ALL => {
                for validator in validator_map.values() {
                    display_validator(validator);
                }
            }
            Ok(name) if name == STR_EXIT => break,
            Ok(name) => {
                let validator = validator_map.get(name).unwrap();
                display_validator(validator);
            }
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        }
    }
    print_divider("Validator");
}

fn examine_object(
    owner_map: &BTreeMap<ObjectID, Owner>,
    validator_pool_id_map: &BTreeMap<ObjectID, &IotaValidatorGenesis>,
    package_map: &BTreeMap<ObjectID, MovePackage>,
    iota_map: &BTreeMap<ObjectID, GasCoin>,
    staked_iota_map: &BTreeMap<ObjectID, StakedIota>,
    timelocked_iota_map: &BTreeMap<ObjectID, TimeLock<Balance>>,
    timelocked_staked_iota: &BTreeMap<ObjectID, TimelockedStakedIota>,
    alias_output_iota_map: &BTreeMap<ObjectID, AliasOutput>,
    basic_output_iota_map: &BTreeMap<ObjectID, BasicOutput>,
    nft_output_iota_map: &BTreeMap<ObjectID, NftOutput>,
    coin_metadata_map: &BTreeMap<ObjectID, CoinMetadata>,
    other_object_map: &BTreeMap<ObjectID, MoveObject>,
) {
    let object_options: Vec<&str> = vec![
        STR_IOTA,
        STR_STAKED_IOTA,
        STR_TIMELOCKED_IOTA,
        STR_TIMELOCKED_STAKED_IOTA,
        STR_ALIAS_OUTPUT_IOTA,
        STR_BASIC_OUTPUT_IOTA,
        STR_NFT_OUTPUT_IOTA,
        STR_COIN_METADATA,
        STR_PACKAGE,
        STR_OTHER,
        STR_EXIT,
    ];
    loop {
        let ans = Select::new(
            "Select one object category to examine ('Exit' to return to Main):",
            object_options.clone(),
        )
        .prompt();
        match ans {
            Ok(name) if name == STR_EXIT => break,
            Ok(name) if name == STR_IOTA => {
                for gas_coin in iota_map.values() {
                    display_iota(gas_coin, owner_map);
                }
                print_divider("Iota");
            }
            Ok(name) if name == STR_STAKED_IOTA => {
                for staked_iota_coin in staked_iota_map.values() {
                    display_staked_iota(staked_iota_coin, validator_pool_id_map, owner_map);
                }
                print_divider(STR_STAKED_IOTA);
            }
            Ok(name) if name == STR_TIMELOCKED_IOTA => {
                for timelocked in timelocked_iota_map.values() {
                    println!("{:#?}", timelocked);
                }
                print_divider(STR_TIMELOCKED_IOTA);
            }
            Ok(name) if name == STR_TIMELOCKED_STAKED_IOTA => {
                for timelocked_staked in timelocked_staked_iota.values() {
                    display_timelocked_staked_iota(
                        timelocked_staked,
                        validator_pool_id_map,
                        owner_map,
                    );
                }
                print_divider(STR_TIMELOCKED_STAKED_IOTA);
            }
            Ok(name) if name == STR_ALIAS_OUTPUT_IOTA => {
                for alias_output in alias_output_iota_map.values() {
                    println!("{:#?}", alias_output);
                }
                print_divider(STR_ALIAS_OUTPUT_IOTA);
            }
            Ok(name) if name == STR_BASIC_OUTPUT_IOTA => {
                for basic_output in basic_output_iota_map.values() {
                    println!("{:#?}", basic_output);
                }
                print_divider(STR_BASIC_OUTPUT_IOTA);
            }
            Ok(name) if name == STR_NFT_OUTPUT_IOTA => {
                for nft_output in nft_output_iota_map.values() {
                    println!("{:#?}", nft_output);
                }
                print_divider(STR_NFT_OUTPUT_IOTA);
            }
            Ok(name) if name == STR_PACKAGE => {
                for package in package_map.values() {
                    println!("Package ID: {}", package.id());
                    println!("Version: {}", package.version());
                    println!("Modules: {:?}\n", package.serialized_module_map().keys());
                }
                print_divider("Package");
            }
            Ok(name) if name == STR_OTHER => {
                for other_obj in other_object_map.values() {
                    println!("{:#?}", other_obj.type_());
                    println!("{:?}", other_obj.version());
                }
                print_divider("Other");
            }
            Ok(name) if name == STR_COIN_METADATA => {
                for coin_metadata in coin_metadata_map.values() {
                    println!("{:#?}\n", coin_metadata);
                }
                print_divider("CoinMetadata");
            }
            Ok(_) => (),
            Err(err) => {
                println!("Error: {err}");
                break;
            }
        }
    }
    print_divider("Object");
}

fn examine_total_supply(
    iota_treasury_cap: &IotaTreasuryCap,
    iota_distribution: &BTreeMap<String, BTreeMap<String, (&str, u64)>>,
    print: bool,
) {
    let mut total_iota = 0;
    let mut total_staked_iota = 0;
    for (owner, coins) in iota_distribution {
        let mut amount_sum = 0;
        for (owner, value) in coins.values() {
            amount_sum += value;
            if *owner == STR_STAKED_IOTA || *owner == STR_TIMELOCKED_STAKED_IOTA {
                total_staked_iota += value;
            }
        }
        total_iota += amount_sum;
        if print {
            println!("Owner {:?}", owner);
            println!(
                "Total Amount of Iota/StakedIota Owned: {amount_sum} NANOS or {} IOTA:",
                amount_sum / NANOS_PER_IOTA
            );
            println!("{:#?}\n", coins);
        }
    }
    // Always print this.
    println!(
        "Total Supply of Iota: {total_iota} NANOS or {} IOTA",
        total_iota / NANOS_PER_IOTA
    );
    println!(
        "Total Amount of StakedIota: {total_staked_iota} NANOS or {} IOTA\n",
        total_staked_iota / NANOS_PER_IOTA
    );
    assert_eq!(total_iota, iota_treasury_cap.total_supply().value);
    if print {
        print_divider("IOTA Distribution");
    }
}

fn display_validator(validator: &IotaValidatorGenesis) {
    let metadata = validator.verified_metadata();
    println!("Validator name: {}", metadata.name);
    println!("{:#?}", metadata);
    println!("Voting Power: {}", validator.voting_power);
    println!("Gas Price: {}", validator.gas_price);
    println!("Next Epoch Gas Price: {}", validator.next_epoch_gas_price);
    println!("Commission Rate: {}", validator.commission_rate);
    println!(
        "Next Epoch Commission Rate: {}",
        validator.next_epoch_commission_rate
    );
    println!("Next Epoch Stake: {}", validator.next_epoch_stake);
    println!("Staking Pool ID: {}", validator.staking_pool.id);
    println!(
        "Staking Pool Activation Epoch: {:?}",
        validator.staking_pool.activation_epoch
    );
    println!(
        "Staking Pool Deactivation Epoch: {:?}",
        validator.staking_pool.deactivation_epoch
    );
    println!(
        "Staking Pool IOTA Balance: {:?}",
        validator.staking_pool.iota_balance
    );
    println!(
        "Rewards Pool: {}",
        validator.staking_pool.rewards_pool.value()
    );
    println!(
        "Pool Token Balance: {}",
        validator.staking_pool.pool_token_balance
    );
    println!(
        "Pending Delegation: {}",
        validator.staking_pool.pending_stake
    );
    println!(
        "Pending Total IOTA Withdraw: {}",
        validator.staking_pool.pending_total_iota_withdraw
    );
    println!(
        "Pending Pool Token Withdraw: {}",
        validator.staking_pool.pending_pool_token_withdraw
    );
    println!(
        "Exchange Rates ID: {}",
        validator.staking_pool.exchange_rates.id
    );
    println!(
        "Exchange Rates Size: {}",
        validator.staking_pool.exchange_rates.size
    );
    print_divider(&metadata.name);
}

fn display_iota(gas_coin: &GasCoin, owner_map: &BTreeMap<ObjectID, Owner>) {
    println!("ID: {}", gas_coin.id());
    println!("Balance: {}", gas_coin.value());
    println!("Owner: {}\n", owner_map.get(gas_coin.id()).unwrap());
}

fn display_staked_iota(
    staked_iota: &StakedIota,
    validator_pool_id_map: &BTreeMap<ObjectID, &IotaValidatorGenesis>,
    owner_map: &BTreeMap<ObjectID, Owner>,
) {
    println!("{:#?}", staked_iota);
    display_staked(
        &staked_iota.pool_id(),
        &staked_iota.id(),
        validator_pool_id_map,
        owner_map,
    );
}

fn display_timelocked_staked_iota(
    timelocked_staked_iota: &TimelockedStakedIota,
    validator_pool_id_map: &BTreeMap<ObjectID, &IotaValidatorGenesis>,
    owner_map: &BTreeMap<ObjectID, Owner>,
) {
    println!("{:#?}", timelocked_staked_iota);
    display_staked(
        &timelocked_staked_iota.pool_id(),
        &timelocked_staked_iota.id(),
        validator_pool_id_map,
        owner_map,
    );
}

fn display_staked(
    pool_id: &ObjectID,
    staked_object_id: &ObjectID,
    validator_pool_id_map: &BTreeMap<ObjectID, &IotaValidatorGenesis>,
    owner_map: &BTreeMap<ObjectID, Owner>,
) {
    let validator = validator_pool_id_map.get(pool_id).unwrap();
    println!(
        "Staked to Validator: {}",
        validator.verified_metadata().name
    );
    println!("Owner: {}\n", owner_map.get(staked_object_id).unwrap());
}

fn print_divider(title: &str) {
    let title = format!("End of {title}");
    let divider_length = 80;
    let left_divider_length = 10;
    assert!(title.len() <= divider_length - left_divider_length * 2);
    let divider_op = "-";
    let divider = divider_op.repeat(divider_length);
    let left_divider = divider_op.repeat(left_divider_length);
    let margin_length = (divider_length - left_divider_length * 2 - title.len()) / 2;
    let margin = " ".repeat(margin_length);
    println!();
    println!("{divider}");
    println!("{left_divider}{margin}{title}{margin}{left_divider}");
    println!("{divider}");
    println!();
}
