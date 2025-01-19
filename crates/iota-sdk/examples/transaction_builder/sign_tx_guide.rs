// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! This example walks through the Rust SDK use case described in
//! https://github.com/iotaledger/iota/blob/develop/docs/content/developer/iota-101/transactions/sign-and-send-txn.mdx
//!
//! cargo run --example sign_tx_guide

#[path = "../utils.rs"]
mod utils;

use anyhow::anyhow;
use fastcrypto::{
    ed25519::Ed25519KeyPair,
    encoding::{Base64, Encoding},
    hash::HashFunction,
    secp256k1::Secp256k1KeyPair,
    secp256r1::Secp256r1KeyPair,
    traits::{EncodeDecodeBase64, KeyPair},
};
use iota_sdk::{
    IotaClientBuilder,
    rpc_types::IotaTransactionBlockResponseOptions,
    types::{
        programmable_transaction_builder::ProgrammableTransactionBuilder,
        transaction::TransactionData,
    },
};
use iota_types::{
    base_types::IotaAddress,
    crypto::{IotaKeyPair, IotaSignature, Signer, ToFromBytes, get_key_pair_from_rng},
    signature::GenericSignature,
};
use rand::{SeedableRng, rngs::StdRng};
use shared_crypto::intent::{Intent, IntentMessage};
use utils::request_tokens_from_faucet;

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // set up iota client for the desired network.
    let client = IotaClientBuilder::default().build_testnet().await?;

    // deterministically generate a keypair, testing only, do not use for mainnet,
    // use the next section to randomly generate a keypair instead.
    let ikp_determ_0 =
        IotaKeyPair::Ed25519(Ed25519KeyPair::generate(&mut StdRng::from_seed([0; 32])));
    let _ikp_determ_1 =
        IotaKeyPair::Secp256k1(Secp256k1KeyPair::generate(&mut StdRng::from_seed([0; 32])));
    let _ikp_determ_2 =
        IotaKeyPair::Secp256r1(Secp256r1KeyPair::generate(&mut StdRng::from_seed([0; 32])));

    // randomly generate a keypair.
    let _ikp_rand_0 = IotaKeyPair::Ed25519(get_key_pair_from_rng(&mut rand::rngs::OsRng).1);
    let _ikp_rand_1 = IotaKeyPair::Secp256k1(get_key_pair_from_rng(&mut rand::rngs::OsRng).1);
    let _ikp_rand_2 = IotaKeyPair::Secp256r1(get_key_pair_from_rng(&mut rand::rngs::OsRng).1);

    // import a keypair from a base64 encoded 32-byte `private key` assuming scheme
    // is Ed25519.
    let _ikp_import_no_flag_0 = IotaKeyPair::Ed25519(Ed25519KeyPair::from_bytes(
        &Base64::decode("1GPhHHkVlF6GrCty2IuBkM+tj/e0jn64ksJ1pc8KPoI=")
            .map_err(|_| anyhow!("Invalid base64"))?,
    )?);
    let _ikp_import_no_flag_1 = IotaKeyPair::Ed25519(Ed25519KeyPair::from_bytes(
        &Base64::decode("1GPhHHkVlF6GrCty2IuBkM+tj/e0jn64ksJ1pc8KPoI=")
            .map_err(|_| anyhow!("Invalid base64"))?,
    )?);
    let _ikp_import_no_flag_2 = IotaKeyPair::Ed25519(Ed25519KeyPair::from_bytes(
        &Base64::decode("1GPhHHkVlF6GrCty2IuBkM+tj/e0jn64ksJ1pc8KPoI=")
            .map_err(|_| anyhow!("Invalid base64"))?,
    )?);

    // import a keypair from a base64 encoded 33-byte `flag || private key`.
    // The signature scheme is determined by the flag.
    let _ikp_import_with_flag_0 =
        IotaKeyPair::decode_base64("ANRj4Rx5FZRehqwrctiLgZDPrY/3tI5+uJLCdaXPCj6C")
            .map_err(|_| anyhow!("Invalid base64"))?;
    let _ikp_import_with_flag_1 =
        IotaKeyPair::decode_base64("AdRj4Rx5FZRehqwrctiLgZDPrY/3tI5+uJLCdaXPCj6C")
            .map_err(|_| anyhow!("Invalid base64"))?;
    let _ikp_import_with_flag_2 =
        IotaKeyPair::decode_base64("AtRj4Rx5FZRehqwrctiLgZDPrY/3tI5+uJLCdaXPCj6C")
            .map_err(|_| anyhow!("Invalid base64"))?;

    // import a keypair from a Bech32 encoded 33-byte `flag || private key`.
    // this is the format of a private key exported from IOTA Wallet or
    // iota.keystore.
    let _ikp_import_with_flag_0 = IotaKeyPair::decode(
        "iotaprivkey1qzdlfxn2qa2lj5uprl8pyhexs02sg2wrhdy7qaq50cqgnffw4c247zslwv6",
    )
    .map_err(|_| anyhow!("Invalid Bech32"))?;
    let _ikp_import_with_flag_1 = IotaKeyPair::decode(
        "iotaprivkey1qqesr6xhua2dkt840v9yefely578q5ad90znnpmhhgpekfvwtxke690adlr",
    )
    .map_err(|_| anyhow!("Invalid Bech32"))?;
    let _ikp_import_with_flag_2 = IotaKeyPair::decode(
        "iotaprivkey1qprzkcs823gcrk7n4hy8pzhntdxakpqk32qwjg9f2wyc3myj78egvhgxjrg",
    )
    .map_err(|_| anyhow!("Invalid Bech32"))?;

    // replace `ikp_determ_0` with the variable names above
    let pk = ikp_determ_0.public();
    let sender = IotaAddress::from(&pk);
    println!("Sender: {sender:?}");

    // make sure the sender has a gas coin as an example.
    request_tokens_from_faucet(sender, &client).await?;
    let gas_coin = client
        .coin_read_api()
        .get_coins(sender, None, None, None)
        .await?
        .data
        .into_iter()
        .next()
        .ok_or(anyhow!("No coins found for sender"))?;

    // construct an example programmable transaction.
    let pt = {
        let mut builder = ProgrammableTransactionBuilder::new();
        builder.pay_iota(vec![sender], vec![1])?;
        builder.finish()
    };

    let gas_budget = 5_000_000;
    let gas_price = client.read_api().get_reference_gas_price().await?;

    // create the transaction data that will be sent to the network.
    let tx_data = TransactionData::new_programmable(
        sender,
        vec![gas_coin.object_ref()],
        pt,
        gas_budget,
        gas_price,
    );

    // derive the digest that the keypair should sign on,
    // i.e. the blake2b hash of `intent || tx_data`.
    let intent_msg = IntentMessage::new(Intent::iota_transaction(), tx_data);
    let raw_tx = bcs::to_bytes(&intent_msg).expect("bcs should not fail");
    let mut hasher = iota_types::crypto::DefaultHash::default();
    hasher.update(raw_tx.clone());
    let digest = hasher.finalize().digest;

    // use IotaKeyPair to sign the digest.
    let iota_sig = ikp_determ_0.sign(&digest);

    // if you would like to verify the signature locally before submission, use this
    // function. if it fails to verify locally, the transaction will fail to
    // execute in IOTA.
    let res = iota_sig.verify_secure(
        &intent_msg,
        sender,
        iota_types::crypto::SignatureScheme::ED25519,
    );
    assert!(res.is_ok());

    // execute the transaction.
    let transaction_response = client
        .quorum_driver_api()
        .execute_transaction_block(
            iota_types::transaction::Transaction::from_generic_sig_data(intent_msg.value, vec![
                GenericSignature::Signature(iota_sig),
            ]),
            IotaTransactionBlockResponseOptions::default(),
            None,
        )
        .await?;

    println!(
        "Transaction executed. Transaction digest: {}",
        transaction_response.digest.base58_encode()
    );
    println!("{transaction_response}");
    Ok(())
}
