use std::{path::PathBuf, time::SystemTime};

use anyhow::{Context, Result, anyhow, ensure};
use iota_keys::keystore::{AccountKeystore, InMemKeystore};
use iota_sdk::types::{base_types::IotaAddress, crypto::SignatureScheme::ED25519};
use iota_types::base_types::{ObjectID, ObjectRef};

use crate::{
    aa_initialization::create_abstract_account,
    cli::{AuthenticatorKind, SubmitMode, TxType, WaitMode},
    registry_state::{AccountState, DeploymentState, load_registry, save_registry},
    tempo_query::print_tempo_traceql_queries,
    tx_type::{submit_aa_tx, submit_standard_tx},
    utils::{
        build_client, canonical_path_str, create_immutable_bench_objects, publish_move_package,
        request_tokens,
    },
};

/// NOTE: dev mnemonic, do NOT use in production environments.
const DEFAULT_MNEMONIC: &str = "rain flip mad lamp owner siren tower buddy wolf shy tray exit glad come dry tent they pond wrist web cliff mixed seek drum";

fn now_unix_ms() -> Result<u128> {
    Ok(SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)?
        .as_millis())
}

fn load_sender() -> Result<(InMemKeystore, IotaAddress)> {
    let mut keystore = InMemKeystore::new_insecure_for_tests(0);
    let sender = keystore
        .import_from_mnemonic(DEFAULT_MNEMONIC, ED25519, None, None)
        .context("import_from_mnemonic failed")?;
    Ok((keystore, sender))
}

fn find_cached_deployment(
    registry: &crate::registry_state::RegistryState,
    rpc: &str,
    pkg_path: &str,
) -> Option<(String, DeploymentState)> {
    registry
        .deployments
        .iter()
        .find(|(_name, d)| d.rpc == rpc && d.package_path == pkg_path)
        .map(|(n, d)| (n.clone(), d.clone()))
}

async fn load_or_publish_deployment<K: AccountKeystore>(
    client: &iota_sdk::IotaClient,
    registry: &mut crate::registry_state::RegistryState,
    sender: IotaAddress,
    keystore: &K,
    rpc: &str,
    aa_package_path: &PathBuf,
    gas_budget: u64,
    use_faucet: bool,
    force_republish: bool,
) -> Result<(String, String, ObjectID, ObjectRef)> {
    let pkg_path = canonical_path_str(aa_package_path);

    if let Some((dep_name, dep)) = find_cached_deployment(registry, rpc, &pkg_path) {
        if !force_republish {
            println!("\n=== Deployment cache hit ===");
            println!("deployment: {dep_name}");
            println!("package_id:  {}", dep.package_id);

            let pkg: ObjectID = dep.package_id.parse().context("bad cached package_id")?;
            let meta_obj_id = dep
                .package_metadata_object_id
                .parse()
                .context("bad cached metadata id")?;
            let meta_ver =
                iota_types::base_types::SequenceNumber::from_u64(dep.package_metadata_version);
            let meta_digest = dep
                .package_metadata_digest
                .parse()
                .context("bad cached metadata digest")?;

            return Ok((
                dep_name,
                dep.publish_tx_digest,
                pkg,
                (meta_obj_id, meta_ver, meta_digest),
            ));
        }

        println!("\n=== Deployment cache hit but force_republish=true ===");
    } else {
        println!("\n=== Deployment cache miss ===");
    }

    if use_faucet {
        println!("Requesting tokens from faucet for sender {sender}");
        request_tokens(client, sender)
            .await
            .context("request_tokens failed (faucet)")?;
        println!("Faucet request completed");
    }

    println!("Publishing Move package: {}", aa_package_path.display());
    let (txd, pkg, meta) =
        publish_move_package(client, sender, keystore, aa_package_path, gas_budget)
            .await
            .context("publish_move_package failed")?;

    let new_name = format!("deploy-{}", now_unix_ms()?);
    registry.deployments.insert(
        new_name.clone(),
        DeploymentState {
            created_at_unix_ms: now_unix_ms()?,
            rpc: rpc.to_string(),
            package_path: pkg_path,
            publish_tx_digest: txd.clone(),
            package_id: pkg.to_string(),
            package_metadata_object_id: meta.0.to_string(),
            package_metadata_version: meta.1.value(),
            package_metadata_digest: meta.2.to_string(),
        },
    );

    Ok((new_name, txd, pkg, meta))
}

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

    let (keystore, sender) = load_sender()?;
    println!("Sender: {sender}");
    println!("RPC: {rpc}");

    let (deployment_name, publish_tx_digest, package_id, metadata_ref) =
        load_or_publish_deployment(
            &client,
            &mut registry,
            sender,
            &keystore,
            &rpc,
            aa_package_path,
            gas_budget,
            use_faucet,
            force_republish,
        )
        .await?;

    let (aa_ref, aa_addr) = create_abstract_account(
        &client,
        sender,
        &keystore,
        package_id,
        metadata_ref,
        authenticator,
        gas_budget,
    )
    .await
    .context("create_abstract_account failed")?;

    let mut bench_objects: Vec<crate::registry_state::StoredObjectRef> = vec![];

    if let Some(spec) = authenticator.bench_init_spec() {
        println!("Authenticator requires bench objects: {spec:?}");

        let bench_refs = create_immutable_bench_objects(
            &client,
            sender,
            &keystore,
            package_id,
            gas_budget,
            spec.entry_fn,
            spec.expected_count,
        )
        .await
        .context("create_immutable_bench_objects failed")?;

        ensure!(
            bench_refs.len() == spec.expected_count,
            "bench object count mismatch: expected={}, got={}",
            spec.expected_count,
            bench_refs.len()
        );

        bench_objects = bench_refs
            .into_iter()
            .map(crate::registry_state::StoredObjectRef::from_object_ref)
            .collect();

        println!("Created {} bench objects.", bench_objects.len());
    }

    registry.accounts.insert(
        name.clone(),
        AccountState {
            created_at_unix_ms: now_unix_ms()?,
            rpc: rpc.clone(),
            sender: sender.to_string(),
            deployment: Some(deployment_name.clone()),
            publish_tx_digest,
            package_id: package_id.to_string(),
            package_metadata_object_id: metadata_ref.0.to_string(),
            aa_account_object_id: aa_ref.0.to_string(),
            aa_account_version: aa_ref.1.value(),
            aa_account_digest: aa_ref.2.to_string(),
            aa_address: aa_addr.to_string(),
            authenticator,
            bench_objects,
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
    tx_type: TxType,
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

    let client = build_client(&acc.rpc).await?;

    let (keystore, sender) = load_sender()?;

    let recipient_addr: IotaAddress = if let Some(r) = recipient {
        r.parse().context("bad recipient address")?
    } else {
        sender
    };

    if use_faucet {
        match mode {
            SubmitMode::Standard => request_tokens(&client, sender)
                .await
                .context("request_tokens(sender) failed")?,
            SubmitMode::Aa => {
                let aa_addr: IotaAddress =
                    acc.aa_address.parse().context("bad aa_address in state")?;
                request_tokens(&client, aa_addr)
                    .await
                    .context("request_tokens(aa_addr) failed")?
            }
        }
    }

    println!("\n=== Submit: mode={mode:?}, count={count}, split_amount={split_amount} ===");

    let wait_mode = wait_mode.to_exec_request();
    let started = std::time::Instant::now();

    let mut digests: Vec<String> = Vec::with_capacity(count);

    for _ in 0..count {
        let r = match mode {
            SubmitMode::Standard => submit_standard_tx(
                &client,
                &keystore,
                sender,
                recipient_addr,
                gas_budget,
                split_amount,
                tx_type,
                wait_mode.clone(),
            )
            .await
            .context("submit_standard_tx failed")?,
            SubmitMode::Aa => submit_aa_tx(
                &client,
                &keystore,
                sender,
                acc,
                recipient_addr,
                gas_budget,
                split_amount,
                tx_type,
                wait_mode.clone(),
            )
            .await
            .context("submit_aa_tx failed")?,
        };

        digests.push(r.digest.clone());
    }

    let tx_sender_for_query = match mode {
        SubmitMode::Standard => sender.to_string(),
        SubmitMode::Aa => acc.aa_address.clone(),
    };

    print_tempo_traceql_queries(
        "iota-node",
        "handle_transaction",
        &tx_sender_for_query,
        &digests,
    );

    let total_ms = started.elapsed().as_millis() as f64;
    let tps = (count as f64) / (total_ms / 1000.0);

    println!("\n=== Batch summary ===");
    println!("count={count} total_ms={total_ms:.2} tps={tps:.2}");

    Ok(())
}
