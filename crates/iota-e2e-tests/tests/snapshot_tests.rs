// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use clap::Parser;
use insta::assert_json_snapshot;
use iota::iota_commands::IotaCommand;
use iota_macros::sim_test;
use iota_sdk::wallet_context::WalletContext;
use test_cluster::TestClusterBuilder;

// special constants for substitution in commands
const ME: &str = "{ME}";

async fn run_one(
    test: Vec<&str>,
    context: &mut WalletContext,
) -> Result<Vec<serde_json::Value>, anyhow::Error> {
    let mut test_output = Vec::new();
    let active_addr = context.active_address()?.to_string();
    for cli_cmd in test {
        let mut cli_cmd_vec = cli_cmd.split(' ').collect::<Vec<&str>>();
        for word in cli_cmd_vec.iter_mut() {
            if *word == ME {
                *word = &active_addr
            }
        }
        test_output.push(serde_json::Value::String(cli_cmd.to_string()));
        let c = IotaCommand::try_parse_from(cli_cmd_vec)?;
        match c {
            IotaCommand::Client { cmd, .. } => {
                if let Some(client_cmd) = cmd {
                    match client_cmd.execute(context).await {
                        Ok(output) => {
                            if let Some(block_response) = output.tx_block_response() {
                                test_output.push(serde_json::to_value(block_response)?);
                            } else if let Some(objects_response) = output.objects_response() {
                                test_output.push(serde_json::to_value(objects_response)?)
                            }
                        }
                        Err(e) => test_output.push(serde_json::Value::String(e.to_string())),
                    }
                }
            }
            IotaCommand::Move { .. } => {
                unimplemented!("Supporting Move publish and upgrade commands")
            }
            _ => panic!("Command {:?} not supported by RPC snapshot tests", cli_cmd),
        }
    }
    Ok(test_output)
}

#[sim_test]
async fn basic_read_cmd_snapshot_tests() -> Result<(), anyhow::Error> {
    let mut test_cluster = TestClusterBuilder::new().build().await;
    let context = &mut test_cluster.wallet;

    // These object ids and transaction digest are picked by executing this test and
    // copying over some random ids from the snapshot file
    let cmds = vec![
        "iota client objects {ME}", // valid addr
        "iota client objects 0x0000000000000000000000000000000000000000000000000000000000000000", /* empty addr */
        "iota client object 0x5",       // valid object
        "iota client object 0x5 --bcs", // valid object BCS
        "iota client object 0x4d03f39deb5e27a76a568adb591da553688e6df6fb053bc9ac069f2bd9495ae3", /* valid object */
        "iota client object 0x4d03f39deb5e27a76a568adb591da553688e6df6fb053bc9ac069f2bd9495ae3 --bcs", /* valid object BCS */
        "iota client object 0x0000000000000000000000000000000000000000000000000000000000000000", /* non-existent object */
        "iota client tx-block 88FqW2hyUgShTyLcGzbh6scZB45XZYmXpXSxydkBVTPu", // valid tx digest
        "iota client tx-block 11111111111111111111111111111111",             /* non-existent tx
                                                                              * digest */
    ];
    assert_json_snapshot!(run_one(cmds, context).await?);
    Ok(())
}
