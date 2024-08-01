// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use std::{io, io::Write, thread::sleep, time::Duration};

use anyhow::anyhow;
use fastcrypto::{
    ed25519::Ed25519KeyPair,
    encoding::{Base64, Encoding},
    jwt_utils::parse_and_validate_jwt,
    traits::{EncodeDecodeBase64, KeyPair},
};
use fastcrypto_zkp::bn254::{
    utils::{gen_address_seed, get_proof, get_salt, get_zk_login_address},
    zk_login::ZkLoginInputs,
};
use iota_json_rpc_types::IotaTransactionBlockResponseOptions;
use iota_keys::keystore::{AccountKeystore, Keystore};
use iota_sdk::IotaClientBuilder;
use iota_types::{
    base_types::IotaAddress,
    committee::EpochId,
    crypto::{IotaKeyPair, PublicKey},
    multisig::{MultiSig, MultiSigPublicKey},
    signature::GenericSignature,
    transaction::Transaction,
    zk_login_authenticator::ZkLoginAuthenticator,
};
use rand::{rngs::StdRng, SeedableRng};
use regex::Regex;
use reqwest::Client;
use serde_json::json;
use shared_crypto::intent::Intent;

/// Read a line from stdin, parse the id_token field and return.
pub fn read_cli_line() -> Result<String, anyhow::Error> {
    let mut s = String::new();
    let _ = io::stdout().flush();
    io::stdin().read_line(&mut s)?;
    let full_url = s.trim_end().to_string();
    let mut parsed_token = "";
    let re = Regex::new(r"id_token=([^&]+)").unwrap();
    if let Some(captures) = re.captures(&full_url) {
        if let Some(id_token) = captures.get(1) {
            parsed_token = id_token.as_str();
        }
    }
    Ok(parsed_token.to_string())
}

/// A util function to request gas token from faucet for the given address.
pub(crate) async fn request_tokens_from_faucet(
    address: IotaAddress,
    gas_url: &str,
) -> Result<(), anyhow::Error> {
    let client = Client::new();
    client
        .post(gas_url)
        .header("Content-Type", "application/json")
        .json(&json![{
            "FixedAmountRequest": {
                "recipient": &address.to_string()
            }
        }])
        .send()
        .await?;
    Ok(())
}

/// A helper function that performs a zklogin test transaction based on the
/// provided parameters.
pub async fn perform_zk_login_test_tx(
    parsed_token: &str,
    max_epoch: EpochId,
    jwt_randomness: &str,
    kp_bigint: &str,
    ephemeral_key_identifier: IotaAddress,
    keystore: &mut Keystore,
    network: &str,
    test_multisig: bool, /* if true, put zklogin in a multisig address with another traditional
                          * pubkey. */
    sign_with_sk: bool, /* if true, submit tx with the traditional sig, otherwise submit with
                         * zklogin sig. */
) -> Result<String, anyhow::Error> {
    let (gas_url, fullnode_url) = get_config(network);
    let user_salt = get_salt(parsed_token, "https://salt.api.mystenlabs.com/get_salt")
        .await
        .unwrap_or("129390038577185583942388216820280642146".to_string());
    println!("User salt: {user_salt}");
    let reader = get_proof(
        parsed_token,
        max_epoch,
        jwt_randomness,
        kp_bigint,
        &user_salt,
        "https://prover-dev.mystenlabs.com/v1",
    )
    .await
    .map_err(|e| anyhow!("Failed to get proof {e}"))?;
    println!("ZkLogin inputs:");
    println!("{:?}", serde_json::to_string(&reader).unwrap());

    let (sub, aud) = parse_and_validate_jwt(parsed_token)?;
    let address_seed = gen_address_seed(&user_salt, "sub", &sub, &aud)?;
    let zk_login_inputs = ZkLoginInputs::from_reader(reader, &address_seed)?;

    let ikp1 = IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([1; 32])));
    let multisig_pk = MultiSigPublicKey::new(
        vec![
            PublicKey::from_zklogin_inputs(&zk_login_inputs)?,
            ikp1.public(),
        ],
        vec![1, 1],
        1,
    )?;

    let sender = if test_multisig {
        keystore.add_key(None, ikp1)?;
        println!("Use multisig address as sender");
        IotaAddress::from(&multisig_pk)
    } else {
        IotaAddress::from_bytes(get_zk_login_address(
            zk_login_inputs.get_address_seed(),
            zk_login_inputs.get_iss(),
        )?)?
    };
    println!("Sender: {:?}", sender);

    // Request some coin from faucet and build a test transaction.
    let iota = IotaClientBuilder::default().build(fullnode_url).await?;
    request_tokens_from_faucet(sender, gas_url).await?;
    sleep(Duration::from_secs(10));

    let Some(coin) = iota
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .next_cursor
    else {
        panic!("Faucet did not work correctly and the provided Iota address has no coins")
    };
    let txb_res = iota
        .transaction_builder()
        .transfer_object(
            sender,
            coin,
            None,
            5000000,
            IotaAddress::ZERO, // as a demo, send to a dummy address
        )
        .await?;
    println!(
        "Faucet requested and created test transaction: {:?}",
        Base64::encode(bcs::to_bytes(&txb_res).unwrap())
    );

    let sig = if sign_with_sk {
        // Create a generic sig from the traditional keypair
        GenericSignature::Signature(keystore.sign_secure(
            &ephemeral_key_identifier,
            &txb_res,
            Intent::iota_transaction(),
        )?)
    } else {
        // Sign transaction with the ephemeral key
        let signature = keystore.sign_secure(
            &ephemeral_key_identifier,
            &txb_res,
            Intent::iota_transaction(),
        )?;

        GenericSignature::from(ZkLoginAuthenticator::new(
            zk_login_inputs,
            max_epoch,
            signature,
        ))
    };

    let multisig = GenericSignature::MultiSig(MultiSig::combine(vec![sig], multisig_pk)?);
    println!("Signature Serialized: {:?}", multisig.encode_base64());

    let transaction_response = iota
        .quorum_driver_api()
        .execute_transaction_block(
            Transaction::from_generic_sig_data(txb_res, vec![multisig]),
            IotaTransactionBlockResponseOptions::full_content(),
            None,
        )
        .await?;
    Ok(transaction_response.digest.base58_encode())
}

fn get_config(network: &str) -> (&str, &str) {
    match network {
        "devnet" => (
            "https://faucet.devnet.iota.io/gas",
            "https://rpc.devnet.iota.io:443",
        ),
        "localnet" => ("http://127.0.0.1:9123/gas", "http://127.0.0.1:9000"),
        _ => panic!("Invalid network"),
    }
}
