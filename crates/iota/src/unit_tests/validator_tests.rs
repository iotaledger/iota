// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{str::FromStr, time::Duration};

use anyhow::Ok;
use iota_json::IotaJsonValue;
use tempfile::TempDir;
use test_cluster::TestClusterBuilder;
use tokio::time::sleep;

use crate::{
    client_commands::{IotaClientCommandResult, IotaClientCommands, OptsWithGas},
    validator_commands::{IotaValidatorCommand, IotaValidatorCommandResponse},
};

#[tokio::test]
async fn test_become_validator() -> Result<(), anyhow::Error> {
    cleanup_fs();
    let config_dir = TempDir::new().unwrap();

    let test_cluster = TestClusterBuilder::new()
        .with_config_dir(config_dir.path().to_path_buf())
        .build()
        .await;

    let mut context = test_cluster.wallet;
    let address = context.active_address()?;
    let client = context.get_client().await?;

    let response = IotaValidatorCommand::MakeValidatorInfo {
        name: "validator0".to_string(),
        description: "description".to_string(),
        image_url: "https://iota.org/logo.png".to_string(),
        project_url: "https://www.iota.org".to_string(),
        host_name: "127.0.0.1".to_string(),
    }
    .execute(&mut context)
    .await?;
    let IotaValidatorCommandResponse::MakeValidatorInfo = response else {
        panic!("Expected MakeValidatorInfo");
    };

    let response = IotaValidatorCommand::BecomeCandidate {
        file: "validator.info".into(),
        gas_budget: None,
    }
    .execute(&mut context)
    .await?;
    let IotaValidatorCommandResponse::BecomeCandidate(_become_candidate_tx) = response else {
        panic!("Expected BecomeCandidate");
    };
    // Wait some time to be sure that the tx is executed
    sleep(Duration::from_secs(2)).await;

    // Get coin and stake
    let coins = client
        .coin_read_api()
        .get_coins(address, None, None, None)
        .await?;
    let stake_result = IotaClientCommands::Call {
        package: "0x3".parse()?,
        module: "iota_system".to_string(),
        function: "request_add_stake".to_string(),
        type_args: vec![],
        gas_price: None,
        args: vec![
            IotaJsonValue::from_str("0x5").unwrap(),
            IotaJsonValue::from_str(&coins.data.first().unwrap().coin_object_id.to_string())
                .unwrap(),
            IotaJsonValue::from_str(&address.to_string()).unwrap(),
        ],
        opts: OptsWithGas::for_testing(None, 1000000000),
    }
    .execute(&mut context)
    .await?;
    let IotaClientCommandResult::TransactionBlock(_) = stake_result else {
        panic!("Expected TransactionBlock");
    };
    // Wait some time to be sure that the tx is executed
    sleep(Duration::from_secs(2)).await;

    let response = IotaValidatorCommand::JoinValidators { gas_budget: None }
        .execute(&mut context)
        .await?;
    let IotaValidatorCommandResponse::JoinValidators(_tx) = response else {
        panic!("Expected JoinValidators");
    };
    sleep(Duration::from_secs(2)).await;

    let response = IotaValidatorCommand::DisplayMetadata {
        validator_address: None,
        json: None,
    }
    .execute(&mut context)
    .await?;
    let IotaValidatorCommandResponse::DisplayMetadata = response else {
        panic!("Expected DisplayMetadata");
    };

    cleanup_fs();
    // These files get generated in IotaValidatorCommand::MakeValidatorInfo in the
    // current directory, so we have to clean them up
    fn cleanup_fs() {
        std::fs::remove_file("validator.info").ok();
        std::fs::remove_file("account.key").ok();
        std::fs::remove_file("authority.key").ok();
        std::fs::remove_file("protocol.key").ok();
        std::fs::remove_file("network.key").ok();
    }
    Ok(())
}
