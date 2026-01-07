use std::{path::PathBuf, time::SystemTime};

use anyhow::{Context, Result, anyhow};
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::types::{base_types::IotaAddress, crypto::SignatureScheme::ED25519};
use iota_types::base_types::{ObjectID, ObjectRef};

use crate::{
    aa_initialization::create_abstract_account,
    cli::{AuthenticatorKind, SubmitMode, WaitMode},
    registry_state::{AccountState, DeploymentState, load_registry, save_registry},
    tempo_query::print_tempo_traceql_queries,
    tx_type::{submit_aa_tx, submit_standard_tx},
    utils::{build_client, canonical_path_str, publish_move_package, request_tokens},
};

const DEFAULT_MNEMONIC: &str = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";

pub async fn handle_init_command(
    registry_path: PathBuf,
    rpc: String,
    name: String,
    aa_package_path: &PathBuf,
    authenticator: AuthenticatorKind,
    gas_budget: u64,
    use_faucet: bool,
    force_republish: bool,
) -> Result<()> {
    let client = build_client(&rpc).await?;
    let mut registry = load_registry(&registry_path)?;

    let mut keystore = InMemKeystore::new_insecure_for_tests(0);
    let sender = keystore
        .import_from_mnemonic(DEFAULT_MNEMONIC, ED25519, None, None)
        .context("import_from_mnemonic failed")?;
    println!("Sender: {sender}");
    println!("RPC: {rpc}");
    let pkg_path = canonical_path_str(&aa_package_path);
    let existing = registry
        .deployments
        .iter()
        .find(|(_name, d)| d.rpc == *rpc && d.package_path == pkg_path)
        .map(|(name, d)| (name.clone(), d.clone()));
    let (deployment_name, publish_tx_digest, package_id, metadata_ref): (
        String,
        String,
        ObjectID,
        ObjectRef,
    ) = if let Some((dep_name, dep)) = existing {
        if !force_republish {
            println!("\n=== Deployment cache hit ===");
            println!("deployment: {dep_name}");
            println!("package_id:  {}", dep.package_id);
            println!(
                "metadata:    {} (v={}, digest={})",
                dep.package_metadata_object_id,
                dep.package_metadata_version,
                dep.package_metadata_digest
            );
            let pkg: ObjectID = dep.package_id.parse()?;
            let meta_obj_id = dep.package_metadata_object_id.parse()?;
            let meta_ver =
                iota_types::base_types::SequenceNumber::from_u64(dep.package_metadata_version);
            let meta_digest = dep.package_metadata_digest.parse()?;
            (
                dep_name,
                dep.publish_tx_digest,
                pkg,
                (meta_obj_id, meta_ver, meta_digest),
            )
        } else {
            println!("\n=== Deployment cache hit BUT force_republish=true, republishing ===");
            let (txd, pkg, meta) =
                publish_move_package(&client, sender, &keystore, &aa_package_path, gas_budget)
                    .await?;
            let new_name = format!(
                "deploy-{}",
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_millis()
            );
            registry.deployments.insert(
                new_name.clone(),
                DeploymentState {
                    created_at_unix_ms: SystemTime::now()
                        .duration_since(SystemTime::UNIX_EPOCH)?
                        .as_millis(),
                    rpc: rpc.clone(),
                    package_path: pkg_path.clone(),
                    publish_tx_digest: txd.clone(),
                    package_id: pkg.to_string(),
                    package_metadata_object_id: meta.0.to_string(),
                    package_metadata_version: meta.1.value(),
                    package_metadata_digest: meta.2.to_string(),
                },
            );
            (new_name, txd, pkg, meta)
        }
    } else {
        if use_faucet {
            println!("Tokens requesting from faucet for sender {sender}");
            request_tokens(&client, sender)
                .await
                .context("request_tokens failed (faucet)")?;
            println!("Faucet request completed");
        }
        println!("\n=== Deployment cache miss, publishing package ===");
        let (txd, pkg, meta) =
            publish_move_package(&client, sender, &keystore, &aa_package_path, gas_budget).await?;
        let new_name = format!(
            "deploy-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis()
        );
        registry.deployments.insert(
            new_name.clone(),
            DeploymentState {
                created_at_unix_ms: SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)?
                    .as_millis(),
                rpc: rpc.clone(),
                package_path: pkg_path.clone(),
                publish_tx_digest: txd.clone(),
                package_id: pkg.to_string(),
                package_metadata_object_id: meta.0.to_string(),
                package_metadata_version: meta.1.value(),
                package_metadata_digest: meta.2.to_string(),
            },
        );
        (new_name, txd, pkg, meta)
    };
    // now create AA account using (package_id, metadata_ref)
    let (aa_ref, aa_addr) = create_abstract_account(
        &client,
        sender,
        &keystore,
        package_id,
        metadata_ref,
        authenticator,
        gas_budget,
    )
    .await?;
    registry.accounts.insert(
        name.clone(),
        AccountState {
            created_at_unix_ms: SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)?
                .as_millis(),
            rpc: rpc.clone(),
            sender: sender.to_string(),
            deployment: Some(deployment_name.clone()),
            publish_tx_digest: publish_tx_digest.clone(),
            package_id: package_id.to_string(),
            package_metadata_object_id: metadata_ref.0.to_string(),
            aa_account_object_id: aa_ref.0.to_string(),
            aa_account_version: aa_ref.1.value(),
            aa_account_digest: aa_ref.2.to_string(),
            aa_address: aa_addr.to_string(),
            authenticator,
        },
    );
    registry.active_account = Some(name.clone());
    save_registry(&registry_path, &registry)?;
    println!("Saved account '{name}' and set active (deployment={deployment_name}).");
    Ok(())
}

pub async fn handle_submit_command(
    registry_path: PathBuf,
    mode: SubmitMode,
    count: usize,
    recipient: Option<String>,
    split_amount: u64,
    account: Option<String>,
    gas_budget: u64,
    use_faucet: bool,
    wait_mode: WaitMode,
) -> Result<()> {
    let registry = load_registry(&registry_path)?;
    let account_name = account
        .or_else(|| registry.active_account.clone())
        .ok_or_else(|| {
            anyhow!("no active account; use `accounts use --name ...` or pass --account")
        })?;

    let acc = registry
        .accounts
        .get(&account_name)
        .ok_or_else(|| anyhow!("account '{account_name}' not found"))?;

    let rpc = acc.rpc.clone();
    let client = build_client(&rpc).await?;

    let mut keystore = InMemKeystore::new_insecure_for_tests(0);
    let sender = keystore
        .import_from_mnemonic(DEFAULT_MNEMONIC, ED25519, None, None)
        .context("import_from_mnemonic failed")?;

    let recipient_addr: IotaAddress = if let Some(r) = recipient {
        r.parse().context("bad recipient address")?
    } else {
        sender
    };

    if use_faucet {
        match mode {
            SubmitMode::Standard => {
                request_tokens(&client, sender).await?;
            }
            SubmitMode::Aa => {
                let aa_addr: IotaAddress =
                    acc.aa_address.parse().context("bad aa_address in state")?;
                request_tokens(&client, aa_addr).await?;
            }
        }
    }

    println!("\n=== Submit: mode={mode:?}, count={count}, split_amount={split_amount} ===");

    let mut lat_ms: Vec<u128> = Vec::with_capacity(count);
    let mut digests: Vec<String> = Vec::with_capacity(count);
    let wait_mode = wait_mode.to_exec_request();
    let started = std::time::Instant::now();
    for i in 0..count {
        let r = match mode {
            SubmitMode::Standard => {
                submit_standard_tx(
                    &client,
                    &keystore,
                    sender,
                    recipient_addr,
                    gas_budget,
                    split_amount,
                    wait_mode.clone(),
                )
                .await?
            }
            SubmitMode::Aa => {
                submit_aa_tx(
                    &client,
                    &keystore,
                    sender,
                    &acc,
                    recipient_addr,
                    gas_budget,
                    split_amount,
                    wait_mode.clone(),
                )
                .await?
            }
        };
        digests.push(r.digest.clone());

        lat_ms.push(r.elapsed_ms);
        // println!(
        //     "[{i}] digest={} elapsed_ms={} gas_used={}",
        //     r.digest,
        //     r.elapsed_ms,
        //     r.gas_used.unwrap_or_else(|| "<none>".to_string())
        // );
    }

    let tx_sender_for_query = match mode {
        SubmitMode::Standard => sender.to_string(),
        SubmitMode::Aa => acc.aa_address.clone(),
    };
    let tempo_service_name = "iota";
    print_tempo_traceql_queries(
        tempo_service_name,
        "handle_transaction",
        &tx_sender_for_query,
        &digests,
    );

    lat_ms.sort();

    let total_ms = started.elapsed().as_millis() as f64;
    let tps = (count as f64) / (total_ms / 1000.0);

    println!("\n=== Batch summary ===");
    println!("count={count} total_ms={total_ms:.2} tps={tps:.2}");
    Ok(())
}
