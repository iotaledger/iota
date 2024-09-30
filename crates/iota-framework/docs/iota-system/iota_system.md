---
title: Module `0x3::iota_system`
---

Iota System State Type Upgrade Guide
<code><a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a></code> is a thin wrapper around <code>IotaSystemStateInner</code> that provides a versioned interface.
The <code><a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a></code> object has a fixed ID 0x5, and the <code>IotaSystemStateInner</code> object is stored as a dynamic field.
There are a few different ways to upgrade the <code>IotaSystemStateInner</code> type:

The simplest and one that doesn't involve a real upgrade is to just add dynamic fields to the <code>extra_fields</code> field
of <code>IotaSystemStateInner</code> or any of its sub type. This is useful when we are in a rush, or making a small change,
or still experimenting a new field.

To properly upgrade the <code>IotaSystemStateInner</code> type, we need to ship a new framework that does the following:
1. Define a new <code>IotaSystemStateInner</code>type (e.g. <code>IotaSystemStateInnerV2</code>).
2. Define a data migration function that migrates the old <code>IotaSystemStateInner</code> to the new one (i.e. IotaSystemStateInnerV2).
3. Replace all uses of <code>IotaSystemStateInner</code> with <code>IotaSystemStateInnerV2</code> in both iota_system.move and iota_system_state_inner.move,
with the exception of the <code><a href="iota_system_state_inner.md#0x3_iota_system_state_inner_create">iota_system_state_inner::create</a></code> function, which should always return the genesis type.
4. Inside <code>load_inner_maybe_upgrade</code> function, check the current version in the wrapper, and if it's not the latest version,
call the data migration function to upgrade the inner object. Make sure to also update the version in the wrapper.
A detailed example can be found in iota/tests/framework_upgrades/mock_iota_systems/shallow_upgrade.
Along with the Move change, we also need to update the Rust code to support the new type. This includes:
1. Define a new <code>IotaSystemStateInner</code> struct type that matches the new Move type, and implement the IotaSystemStateTrait.
2. Update the <code><a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a></code> struct to include the new version as a new enum variant.
3. Update the <code>get_iota_system_state</code> function to handle the new version.
To test that the upgrade will be successful, we need to modify <code>iota_system_state_production_upgrade_test</code> test in
protocol_version_tests and trigger a real upgrade using the new framework. We will need to keep this directory as old version,
put the new framework in a new directory, and run the test to exercise the upgrade.

To upgrade Validator type, besides everything above, we also need to:
1. Define a new Validator type (e.g. ValidatorV2).
2. Define a data migration function that migrates the old Validator to the new one (i.e. ValidatorV2).
3. Replace all uses of Validator with ValidatorV2 except the genesis creation function.
4. In validator_wrapper::upgrade_to_latest, check the current version in the wrapper, and if it's not the latest version,
call the data migration function to upgrade it.
In Rust, we also need to add a new case in <code>get_validator_from_table</code>.
Note that it is possible to upgrade IotaSystemStateInner without upgrading Validator, but not the other way around.
And when we only upgrade IotaSystemStateInner, the version of Validator in the wrapper will not be updated, and hence may become
inconsistent with the version of IotaSystemStateInner. This is fine as long as we don't use the Validator version to determine
the IotaSystemStateInner version, or vice versa.


-  [Resource `IotaSystemState`](#0x3_iota_system_IotaSystemState)
-  [Constants](#@Constants_0)
-  [Function `create`](#0x3_iota_system_create)
-  [Function `request_add_validator_candidate`](#0x3_iota_system_request_add_validator_candidate)
-  [Function `request_remove_validator_candidate`](#0x3_iota_system_request_remove_validator_candidate)
-  [Function `request_add_validator`](#0x3_iota_system_request_add_validator)
-  [Function `request_remove_validator`](#0x3_iota_system_request_remove_validator)
-  [Function `request_set_gas_price`](#0x3_iota_system_request_set_gas_price)
-  [Function `set_candidate_validator_gas_price`](#0x3_iota_system_set_candidate_validator_gas_price)
-  [Function `request_set_commission_rate`](#0x3_iota_system_request_set_commission_rate)
-  [Function `set_candidate_validator_commission_rate`](#0x3_iota_system_set_candidate_validator_commission_rate)
-  [Function `request_add_stake`](#0x3_iota_system_request_add_stake)
-  [Function `request_add_stake_non_entry`](#0x3_iota_system_request_add_stake_non_entry)
-  [Function `request_add_stake_mul_coin`](#0x3_iota_system_request_add_stake_mul_coin)
-  [Function `request_withdraw_stake`](#0x3_iota_system_request_withdraw_stake)
-  [Function `request_withdraw_stake_non_entry`](#0x3_iota_system_request_withdraw_stake_non_entry)
-  [Function `report_validator`](#0x3_iota_system_report_validator)
-  [Function `undo_report_validator`](#0x3_iota_system_undo_report_validator)
-  [Function `rotate_operation_cap`](#0x3_iota_system_rotate_operation_cap)
-  [Function `update_validator_name`](#0x3_iota_system_update_validator_name)
-  [Function `update_validator_description`](#0x3_iota_system_update_validator_description)
-  [Function `update_validator_image_url`](#0x3_iota_system_update_validator_image_url)
-  [Function `update_validator_project_url`](#0x3_iota_system_update_validator_project_url)
-  [Function `update_validator_next_epoch_network_address`](#0x3_iota_system_update_validator_next_epoch_network_address)
-  [Function `update_candidate_validator_network_address`](#0x3_iota_system_update_candidate_validator_network_address)
-  [Function `update_validator_next_epoch_p2p_address`](#0x3_iota_system_update_validator_next_epoch_p2p_address)
-  [Function `update_candidate_validator_p2p_address`](#0x3_iota_system_update_candidate_validator_p2p_address)
-  [Function `update_validator_next_epoch_primary_address`](#0x3_iota_system_update_validator_next_epoch_primary_address)
-  [Function `update_candidate_validator_primary_address`](#0x3_iota_system_update_candidate_validator_primary_address)
-  [Function `update_validator_next_epoch_worker_address`](#0x3_iota_system_update_validator_next_epoch_worker_address)
-  [Function `update_candidate_validator_worker_address`](#0x3_iota_system_update_candidate_validator_worker_address)
-  [Function `update_validator_next_epoch_protocol_pubkey`](#0x3_iota_system_update_validator_next_epoch_protocol_pubkey)
-  [Function `update_candidate_validator_protocol_pubkey`](#0x3_iota_system_update_candidate_validator_protocol_pubkey)
-  [Function `update_validator_next_epoch_worker_pubkey`](#0x3_iota_system_update_validator_next_epoch_worker_pubkey)
-  [Function `update_candidate_validator_worker_pubkey`](#0x3_iota_system_update_candidate_validator_worker_pubkey)
-  [Function `update_validator_next_epoch_network_pubkey`](#0x3_iota_system_update_validator_next_epoch_network_pubkey)
-  [Function `update_candidate_validator_network_pubkey`](#0x3_iota_system_update_candidate_validator_network_pubkey)
-  [Function `pool_exchange_rates`](#0x3_iota_system_pool_exchange_rates)
-  [Function `active_validator_addresses`](#0x3_iota_system_active_validator_addresses)
-  [Function `advance_epoch`](#0x3_iota_system_advance_epoch)
-  [Function `load_system_state`](#0x3_iota_system_load_system_state)
-  [Function `load_system_state_mut`](#0x3_iota_system_load_system_state_mut)
-  [Function `load_inner_maybe_upgrade`](#0x3_iota_system_load_inner_maybe_upgrade)
-  [Function `load_system_timelock_cap`](#0x3_iota_system_load_system_timelock_cap)
-  [Function `validator_voting_powers`](#0x3_iota_system_validator_voting_powers)
-  [Function `get_total_iota_supply`](#0x3_iota_system_get_total_iota_supply)


<pre><code><b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field">0x2::dynamic_field</a>;
<b>use</b> <a href="../iota-framework/iota.md#0x2_iota">0x2::iota</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/table.md#0x2_table">0x2::table</a>;
<b>use</b> <a href="../iota-framework/timelock.md#0x2_timelock">0x2::timelock</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-framework/vec_map.md#0x2_vec_map">0x2::vec_map</a>;
<b>use</b> <a href="iota_system_state_inner.md#0x3_iota_system_state_inner">0x3::iota_system_state_inner</a>;
<b>use</b> <a href="staking_pool.md#0x3_staking_pool">0x3::staking_pool</a>;
<b>use</b> <a href="validator.md#0x3_validator">0x3::validator</a>;
<b>use</b> <a href="validator_cap.md#0x3_validator_cap">0x3::validator_cap</a>;
</code></pre>



<a name="0x3_iota_system_IotaSystemState"></a>

## Resource `IotaSystemState`



<pre><code><b>struct</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>

</dd>
<dt>
<code>version: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x3_iota_system_ENotSystemAddress"></a>



<pre><code><b>const</b> <a href="iota_system.md#0x3_iota_system_ENotSystemAddress">ENotSystemAddress</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x3_iota_system_EWrongInnerVersion"></a>



<pre><code><b>const</b> <a href="iota_system.md#0x3_iota_system_EWrongInnerVersion">EWrongInnerVersion</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1;
</code></pre>



<a name="0x3_iota_system_SYSTEM_TIMELOCK_CAP_DF_KEY"></a>



<pre><code><b>const</b> <a href="iota_system.md#0x3_iota_system_SYSTEM_TIMELOCK_CAP_DF_KEY">SYSTEM_TIMELOCK_CAP_DF_KEY</a>: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt; = [115, 121, 115, 95, 116, 105, 109, 101, 108, 111, 99, 107, 95, 99, 97, 112];
</code></pre>



<a name="0x3_iota_system_create"></a>

## Function `create`

Create a new IotaSystemState object and make it shared.
This function will be called only once in genesis.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="iota_system.md#0x3_iota_system_create">create</a>(id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a>, iota_treasury_cap: <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>, validators: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="validator.md#0x3_validator_Validator">validator::Validator</a>&gt;, <a href="storage_fund.md#0x3_storage_fund">storage_fund</a>: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, protocol_version: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, epoch_start_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, parameters: <a href="iota_system_state_inner.md#0x3_iota_system_state_inner_SystemParameters">iota_system_state_inner::SystemParameters</a>, system_timelock_cap: <a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">timelock::SystemTimelockCap</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="iota_system.md#0x3_iota_system_create">create</a>(
    id: UID,
    iota_treasury_cap: IotaTreasuryCap,
    validators: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;Validator&gt;,
    <a href="storage_fund.md#0x3_storage_fund">storage_fund</a>: Balance&lt;IOTA&gt;,
    protocol_version: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    epoch_start_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    parameters: SystemParameters,
    system_timelock_cap: SystemTimelockCap,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> system_state = <a href="iota_system_state_inner.md#0x3_iota_system_state_inner_create">iota_system_state_inner::create</a>(
        iota_treasury_cap,
        validators,
        <a href="storage_fund.md#0x3_storage_fund">storage_fund</a>,
        protocol_version,
        epoch_start_timestamp_ms,
        parameters,
        ctx,
    );
    <b>let</b> version = <a href="iota_system_state_inner.md#0x3_iota_system_state_inner_genesis_system_state_version">iota_system_state_inner::genesis_system_state_version</a>();
    <b>let</b> <b>mut</b> self = <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a> {
        id,
        version,
    };
    <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field_add">dynamic_field::add</a>(&<b>mut</b> self.id, version, system_state);
    <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field_add">dynamic_field::add</a>(&<b>mut</b> self.id, <a href="iota_system.md#0x3_iota_system_SYSTEM_TIMELOCK_CAP_DF_KEY">SYSTEM_TIMELOCK_CAP_DF_KEY</a>, system_timelock_cap);
    <a href="../iota-framework/transfer.md#0x2_transfer_share_object">transfer::share_object</a>(self);
}
</code></pre>



</details>

<a name="0x3_iota_system_request_add_validator_candidate"></a>

## Function `request_add_validator_candidate`

Can be called by anyone who wishes to become a validator candidate and starts accuring delegated
stakes in their staking pool. Once they have at least <code>MIN_VALIDATOR_JOINING_STAKE</code> amount of stake they
can call <code>request_add_validator</code> to officially become an active validator at the next epoch.
Aborts if the caller is already a pending or active validator, or a validator candidate.
Note: <code>proof_of_possession</code> MUST be a valid signature using iota_address and protocol_pubkey_bytes.
To produce a valid PoP, run [fn test_proof_of_possession].


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_validator_candidate">request_add_validator_candidate</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, pubkey_bytes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, network_pubkey_bytes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, worker_pubkey_bytes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, proof_of_possession: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, name: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, description: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, image_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, project_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, net_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, p2p_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, primary_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, worker_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, gas_price: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, commission_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_validator_candidate">request_add_validator_candidate</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    pubkey_bytes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    network_pubkey_bytes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    worker_pubkey_bytes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    proof_of_possession: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    name: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    description: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    image_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    project_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    net_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    p2p_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    primary_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    worker_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    gas_price: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    commission_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_add_validator_candidate">request_add_validator_candidate</a>(
        pubkey_bytes,
        network_pubkey_bytes,
        worker_pubkey_bytes,
        proof_of_possession,
        name,
        description,
        image_url,
        project_url,
        net_address,
        p2p_address,
        primary_address,
        worker_address,
        gas_price,
        commission_rate,
        ctx,
    )
}
</code></pre>



</details>

<a name="0x3_iota_system_request_remove_validator_candidate"></a>

## Function `request_remove_validator_candidate`

Called by a validator candidate to remove themselves from the candidacy. After this call
their staking pool becomes deactivate.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_remove_validator_candidate">request_remove_validator_candidate</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_remove_validator_candidate">request_remove_validator_candidate</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_remove_validator_candidate">request_remove_validator_candidate</a>(ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_request_add_validator"></a>

## Function `request_add_validator`

Called by a validator candidate to add themselves to the active validator set beginning next epoch.
Aborts if the validator is a duplicate with one of the pending or active validators, or if the amount of
stake the validator has doesn't meet the min threshold, or if the number of new validators for the next
epoch has already reached the maximum.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_validator">request_add_validator</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_validator">request_add_validator</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_add_validator">request_add_validator</a>(ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_request_remove_validator"></a>

## Function `request_remove_validator`

A validator can call this function to request a removal in the next epoch.
We use the sender of <code>ctx</code> to look up the validator
(i.e. sender must match the iota_address in the validator).
At the end of the epoch, the <code><a href="validator.md#0x3_validator">validator</a></code> object will be returned to the iota_address
of the validator.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_remove_validator">request_remove_validator</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_remove_validator">request_remove_validator</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_remove_validator">request_remove_validator</a>(ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_request_set_gas_price"></a>

## Function `request_set_gas_price`

A validator can call this entry function to submit a new gas price quote, to be
used for the reference gas price calculation at the end of the epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_set_gas_price">request_set_gas_price</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, cap: &<a href="validator_cap.md#0x3_validator_cap_UnverifiedValidatorOperationCap">validator_cap::UnverifiedValidatorOperationCap</a>, new_gas_price: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_set_gas_price">request_set_gas_price</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    cap: &UnverifiedValidatorOperationCap,
    new_gas_price: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_set_gas_price">request_set_gas_price</a>(cap, new_gas_price)
}
</code></pre>



</details>

<a name="0x3_iota_system_set_candidate_validator_gas_price"></a>

## Function `set_candidate_validator_gas_price`

This entry function is used to set new gas price for candidate validators


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_set_candidate_validator_gas_price">set_candidate_validator_gas_price</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, cap: &<a href="validator_cap.md#0x3_validator_cap_UnverifiedValidatorOperationCap">validator_cap::UnverifiedValidatorOperationCap</a>, new_gas_price: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_set_candidate_validator_gas_price">set_candidate_validator_gas_price</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    cap: &UnverifiedValidatorOperationCap,
    new_gas_price: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_set_candidate_validator_gas_price">set_candidate_validator_gas_price</a>(cap, new_gas_price)
}
</code></pre>



</details>

<a name="0x3_iota_system_request_set_commission_rate"></a>

## Function `request_set_commission_rate`

A validator can call this entry function to set a new commission rate, updated at the end of
the epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_set_commission_rate">request_set_commission_rate</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, new_commission_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_set_commission_rate">request_set_commission_rate</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    new_commission_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_set_commission_rate">request_set_commission_rate</a>(new_commission_rate, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_set_candidate_validator_commission_rate"></a>

## Function `set_candidate_validator_commission_rate`

This entry function is used to set new commission rate for candidate validators


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_set_candidate_validator_commission_rate">set_candidate_validator_commission_rate</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, new_commission_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_set_candidate_validator_commission_rate">set_candidate_validator_commission_rate</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    new_commission_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_set_candidate_validator_commission_rate">set_candidate_validator_commission_rate</a>(new_commission_rate, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_request_add_stake"></a>

## Function `request_add_stake`

Add stake to a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_stake">request_add_stake</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, stake: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_stake">request_add_stake</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    stake: Coin&lt;IOTA&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> staked_iota = <a href="iota_system.md#0x3_iota_system_request_add_stake_non_entry">request_add_stake_non_entry</a>(wrapper, stake, validator_address, ctx);
    <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(staked_iota, ctx.sender());
}
</code></pre>



</details>

<a name="0x3_iota_system_request_add_stake_non_entry"></a>

## Function `request_add_stake_non_entry`

The non-entry version of <code>request_add_stake</code>, which returns the staked IOTA instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_stake_non_entry">request_add_stake_non_entry</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, stake: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_stake_non_entry">request_add_stake_non_entry</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    stake: Coin&lt;IOTA&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
): StakedIota {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_add_stake">request_add_stake</a>(stake, validator_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_request_add_stake_mul_coin"></a>

## Function `request_add_stake_mul_coin`

Add stake to a validator's staking pool using multiple coins.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_stake_mul_coin">request_add_stake_mul_coin</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, stake_amount: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_add_stake_mul_coin">request_add_stake_mul_coin</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;Coin&lt;IOTA&gt;&gt;,
    stake_amount: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    <b>let</b> staked_iota = self.<a href="iota_system.md#0x3_iota_system_request_add_stake_mul_coin">request_add_stake_mul_coin</a>(stakes, stake_amount, validator_address, ctx);
    <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(staked_iota, ctx.sender());
}
</code></pre>



</details>

<a name="0x3_iota_system_request_withdraw_stake"></a>

## Function `request_withdraw_stake`

Withdraw stake from a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_withdraw_stake">request_withdraw_stake</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_withdraw_stake">request_withdraw_stake</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    staked_iota: StakedIota,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> withdrawn_stake = <a href="iota_system.md#0x3_iota_system_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(wrapper, staked_iota, ctx);
    <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(withdrawn_stake.into_coin(ctx), ctx.sender());
}
</code></pre>



</details>

<a name="0x3_iota_system_request_withdraw_stake_non_entry"></a>

## Function `request_withdraw_stake_non_entry`

Non-entry version of <code>request_withdraw_stake</code> that returns the withdrawn IOTA instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    staked_iota: StakedIota,
    ctx: &<b>mut</b> TxContext,
) : Balance&lt;IOTA&gt; {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_request_withdraw_stake">request_withdraw_stake</a>(staked_iota, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_report_validator"></a>

## Function `report_validator`

Report a validator as a bad or non-performant actor in the system.
Succeeds if all the following are satisfied:
1. both the reporter in <code>cap</code> and the input <code>reportee_addr</code> are active validators.
2. reporter and reportee not the same address.
3. the cap object is still valid.
This function is idempotent.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_report_validator">report_validator</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, cap: &<a href="validator_cap.md#0x3_validator_cap_UnverifiedValidatorOperationCap">validator_cap::UnverifiedValidatorOperationCap</a>, reportee_addr: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_report_validator">report_validator</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    cap: &UnverifiedValidatorOperationCap,
    reportee_addr: <b>address</b>,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_report_validator">report_validator</a>(cap, reportee_addr)
}
</code></pre>



</details>

<a name="0x3_iota_system_undo_report_validator"></a>

## Function `undo_report_validator`

Undo a <code>report_validator</code> action. Aborts if
1. the reportee is not a currently active validator or
2. the sender has not previously reported the <code>reportee_addr</code>, or
3. the cap is not valid


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_undo_report_validator">undo_report_validator</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, cap: &<a href="validator_cap.md#0x3_validator_cap_UnverifiedValidatorOperationCap">validator_cap::UnverifiedValidatorOperationCap</a>, reportee_addr: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_undo_report_validator">undo_report_validator</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    cap: &UnverifiedValidatorOperationCap,
    reportee_addr: <b>address</b>,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_undo_report_validator">undo_report_validator</a>(cap, reportee_addr)
}
</code></pre>



</details>

<a name="0x3_iota_system_rotate_operation_cap"></a>

## Function `rotate_operation_cap`

Create a new <code>UnverifiedValidatorOperationCap</code>, transfer it to the
validator and registers it. The original object is thus revoked.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_rotate_operation_cap">rotate_operation_cap</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_rotate_operation_cap">rotate_operation_cap</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_rotate_operation_cap">rotate_operation_cap</a>(ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_name"></a>

## Function `update_validator_name`

Update a validator's name.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_name">update_validator_name</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, name: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_name">update_validator_name</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    name: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_name">update_validator_name</a>(name, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_description"></a>

## Function `update_validator_description`

Update a validator's description


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_description">update_validator_description</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, description: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_description">update_validator_description</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    description: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_description">update_validator_description</a>(description, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_image_url"></a>

## Function `update_validator_image_url`

Update a validator's image url


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_image_url">update_validator_image_url</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, image_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_image_url">update_validator_image_url</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    image_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_image_url">update_validator_image_url</a>(image_url, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_project_url"></a>

## Function `update_validator_project_url`

Update a validator's project url


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_project_url">update_validator_project_url</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, project_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_project_url">update_validator_project_url</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    project_url: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_project_url">update_validator_project_url</a>(project_url, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_network_address"></a>

## Function `update_validator_next_epoch_network_address`

Update a validator's network address.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_network_address">update_validator_next_epoch_network_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, network_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_network_address">update_validator_next_epoch_network_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    network_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_network_address">update_validator_next_epoch_network_address</a>(network_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_network_address"></a>

## Function `update_candidate_validator_network_address`

Update candidate validator's network address.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_network_address">update_candidate_validator_network_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, network_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_network_address">update_candidate_validator_network_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    network_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_network_address">update_candidate_validator_network_address</a>(network_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_p2p_address"></a>

## Function `update_validator_next_epoch_p2p_address`

Update a validator's p2p address.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_p2p_address">update_validator_next_epoch_p2p_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, p2p_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_p2p_address">update_validator_next_epoch_p2p_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    p2p_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_p2p_address">update_validator_next_epoch_p2p_address</a>(p2p_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_p2p_address"></a>

## Function `update_candidate_validator_p2p_address`

Update candidate validator's p2p address.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_p2p_address">update_candidate_validator_p2p_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, p2p_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_p2p_address">update_candidate_validator_p2p_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    p2p_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_p2p_address">update_candidate_validator_p2p_address</a>(p2p_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_primary_address"></a>

## Function `update_validator_next_epoch_primary_address`

Update a validator's narwhal primary address.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_primary_address">update_validator_next_epoch_primary_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, primary_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_primary_address">update_validator_next_epoch_primary_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    primary_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_primary_address">update_validator_next_epoch_primary_address</a>(primary_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_primary_address"></a>

## Function `update_candidate_validator_primary_address`

Update candidate validator's narwhal primary address.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_primary_address">update_candidate_validator_primary_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, primary_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_primary_address">update_candidate_validator_primary_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    primary_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_primary_address">update_candidate_validator_primary_address</a>(primary_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_worker_address"></a>

## Function `update_validator_next_epoch_worker_address`

Update a validator's narwhal worker address.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_worker_address">update_validator_next_epoch_worker_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, worker_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_worker_address">update_validator_next_epoch_worker_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    worker_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_worker_address">update_validator_next_epoch_worker_address</a>(worker_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_worker_address"></a>

## Function `update_candidate_validator_worker_address`

Update candidate validator's narwhal worker address.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_worker_address">update_candidate_validator_worker_address</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, worker_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_worker_address">update_candidate_validator_worker_address</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    worker_address: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_worker_address">update_candidate_validator_worker_address</a>(worker_address, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_protocol_pubkey"></a>

## Function `update_validator_next_epoch_protocol_pubkey`

Update a validator's public key of protocol key and proof of possession.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_protocol_pubkey">update_validator_next_epoch_protocol_pubkey</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, protocol_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, proof_of_possession: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_protocol_pubkey">update_validator_next_epoch_protocol_pubkey</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    protocol_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    proof_of_possession: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_protocol_pubkey">update_validator_next_epoch_protocol_pubkey</a>(protocol_pubkey, proof_of_possession, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_protocol_pubkey"></a>

## Function `update_candidate_validator_protocol_pubkey`

Update candidate validator's public key of protocol key and proof of possession.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_protocol_pubkey">update_candidate_validator_protocol_pubkey</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, protocol_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, proof_of_possession: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_protocol_pubkey">update_candidate_validator_protocol_pubkey</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    protocol_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    proof_of_possession: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_protocol_pubkey">update_candidate_validator_protocol_pubkey</a>(protocol_pubkey, proof_of_possession, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_worker_pubkey"></a>

## Function `update_validator_next_epoch_worker_pubkey`

Update a validator's public key of worker key.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_worker_pubkey">update_validator_next_epoch_worker_pubkey</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, worker_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_worker_pubkey">update_validator_next_epoch_worker_pubkey</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    worker_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_worker_pubkey">update_validator_next_epoch_worker_pubkey</a>(worker_pubkey, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_worker_pubkey"></a>

## Function `update_candidate_validator_worker_pubkey`

Update candidate validator's public key of worker key.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_worker_pubkey">update_candidate_validator_worker_pubkey</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, worker_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_worker_pubkey">update_candidate_validator_worker_pubkey</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    worker_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_worker_pubkey">update_candidate_validator_worker_pubkey</a>(worker_pubkey, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_validator_next_epoch_network_pubkey"></a>

## Function `update_validator_next_epoch_network_pubkey`

Update a validator's public key of network key.
The change will only take effects starting from the next epoch.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_network_pubkey">update_validator_next_epoch_network_pubkey</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, network_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_network_pubkey">update_validator_next_epoch_network_pubkey</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    network_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_validator_next_epoch_network_pubkey">update_validator_next_epoch_network_pubkey</a>(network_pubkey, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_update_candidate_validator_network_pubkey"></a>

## Function `update_candidate_validator_network_pubkey`

Update candidate validator's public key of network key.


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_network_pubkey">update_candidate_validator_network_pubkey</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, network_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="iota_system.md#0x3_iota_system_update_candidate_validator_network_pubkey">update_candidate_validator_network_pubkey</a>(
    self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    network_pubkey: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;,
    ctx: &TxContext,
) {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self);
    self.<a href="iota_system.md#0x3_iota_system_update_candidate_validator_network_pubkey">update_candidate_validator_network_pubkey</a>(network_pubkey, ctx)
}
</code></pre>



</details>

<a name="0x3_iota_system_pool_exchange_rates"></a>

## Function `pool_exchange_rates`

Getter of the pool token exchange rate of a staking pool. Works for both active and inactive pools.


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_pool_exchange_rates">pool_exchange_rates</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, pool_id: &<a href="../iota-framework/object.md#0x2_object_ID">object::ID</a>): &<a href="../iota-framework/table.md#0x2_table_Table">table::Table</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_pool_exchange_rates">pool_exchange_rates</a>(
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    pool_id: &ID
): &Table&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, PoolTokenExchangeRate&gt;  {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_pool_exchange_rates">pool_exchange_rates</a>(pool_id)
}
</code></pre>



</details>

<a name="0x3_iota_system_active_validator_addresses"></a>

## Function `active_validator_addresses`

Getter returning addresses of the currently active validators.


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_active_validator_addresses">active_validator_addresses</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<b>address</b>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_active_validator_addresses">active_validator_addresses</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<b>address</b>&gt; {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state">load_system_state</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_active_validator_addresses">active_validator_addresses</a>()
}
</code></pre>



</details>

<a name="0x3_iota_system_advance_epoch"></a>

## Function `advance_epoch`

This function should be called at the end of an epoch, and advances the system to the next epoch.
It does the following things:
1. Add storage charge to the storage fund.
2. Burn the storage rebates from the storage fund. These are already refunded to transaction sender's
gas coins.
3. Mint or burn IOTA tokens depending on whether the validator target reward is greater
or smaller than the computation reward.
4. Distribute the target reward to the validators.
5. Burn any leftover rewards.
6. Update all validators.


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_advance_epoch">advance_epoch</a>(validator_target_reward: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, storage_charge: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, computation_reward: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, new_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, next_protocol_version: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, storage_rebate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, non_refundable_storage_fee: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, reward_slashing_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, epoch_start_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_advance_epoch">advance_epoch</a>(
    validator_target_reward: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    storage_charge: Balance&lt;IOTA&gt;,
    computation_reward: Balance&lt;IOTA&gt;,
    wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>,
    new_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    next_protocol_version: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    storage_rebate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    non_refundable_storage_fee: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    reward_slashing_rate: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, // how much rewards are slashed <b>to</b> punish a <a href="validator.md#0x3_validator">validator</a>, in bps.
    epoch_start_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, // Timestamp of the epoch start
    ctx: &<b>mut</b> TxContext,
) : Balance&lt;IOTA&gt; {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(wrapper);
    // Validator will make a special system call <b>with</b> sender set <b>as</b> 0x0.
    <b>assert</b>!(ctx.sender() == @0x0, <a href="iota_system.md#0x3_iota_system_ENotSystemAddress">ENotSystemAddress</a>);
    <b>let</b> storage_rebate = self.<a href="iota_system.md#0x3_iota_system_advance_epoch">advance_epoch</a>(
        new_epoch,
        next_protocol_version,
        validator_target_reward,
        storage_charge,
        computation_reward,
        storage_rebate,
        non_refundable_storage_fee,
        reward_slashing_rate,
        epoch_start_timestamp_ms,
        ctx,
    );

    storage_rebate
}
</code></pre>



</details>

<a name="0x3_iota_system_load_system_state"></a>

## Function `load_system_state`



<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_load_system_state">load_system_state</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): &<a href="iota_system_state_inner.md#0x3_iota_system_state_inner_IotaSystemStateInnerV2">iota_system_state_inner::IotaSystemStateInnerV2</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_load_system_state">load_system_state</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): &IotaSystemStateInnerV2 {
    <a href="iota_system.md#0x3_iota_system_load_inner_maybe_upgrade">load_inner_maybe_upgrade</a>(self)
}
</code></pre>



</details>

<a name="0x3_iota_system_load_system_state_mut"></a>

## Function `load_system_state_mut`



<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): &<b>mut</b> <a href="iota_system_state_inner.md#0x3_iota_system_state_inner_IotaSystemStateInnerV2">iota_system_state_inner::IotaSystemStateInnerV2</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_load_system_state_mut">load_system_state_mut</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): &<b>mut</b> IotaSystemStateInnerV2 {
    <a href="iota_system.md#0x3_iota_system_load_inner_maybe_upgrade">load_inner_maybe_upgrade</a>(self)
}
</code></pre>



</details>

<a name="0x3_iota_system_load_inner_maybe_upgrade"></a>

## Function `load_inner_maybe_upgrade`



<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_load_inner_maybe_upgrade">load_inner_maybe_upgrade</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): &<b>mut</b> <a href="iota_system_state_inner.md#0x3_iota_system_state_inner_IotaSystemStateInnerV2">iota_system_state_inner::IotaSystemStateInnerV2</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_load_inner_maybe_upgrade">load_inner_maybe_upgrade</a>(self: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): &<b>mut</b> IotaSystemStateInnerV2 {
    <b>if</b> (self.version == 1) {
        <b>let</b> v1: IotaSystemStateInner = <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field_remove">dynamic_field::remove</a>(&<b>mut</b> self.id, self.version);
        <b>let</b> v2 = v1.v1_to_v2();
        self.version = 2;
        <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field_add">dynamic_field::add</a>(&<b>mut</b> self.id, self.version, v2);
    };

    <b>let</b> inner: &<b>mut</b> IotaSystemStateInnerV2 = <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field_borrow_mut">dynamic_field::borrow_mut</a>(
        &<b>mut</b> self.id,
        self.version
    );
    <b>assert</b>!(inner.system_state_version() == self.version, <a href="iota_system.md#0x3_iota_system_EWrongInnerVersion">EWrongInnerVersion</a>);
    inner
}
</code></pre>



</details>

<a name="0x3_iota_system_load_system_timelock_cap"></a>

## Function `load_system_timelock_cap`



<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="iota_system.md#0x3_iota_system_load_system_timelock_cap">load_system_timelock_cap</a>(self: &<a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): &<a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">timelock::SystemTimelockCap</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="iota_system.md#0x3_iota_system_load_system_timelock_cap">load_system_timelock_cap</a>(self: &<a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): &SystemTimelockCap {
    <a href="../iota-framework/dynamic_field.md#0x2_dynamic_field_borrow">dynamic_field::borrow</a>(
        &self.id,
        <a href="iota_system.md#0x3_iota_system_SYSTEM_TIMELOCK_CAP_DF_KEY">SYSTEM_TIMELOCK_CAP_DF_KEY</a>
    )
}
</code></pre>



</details>

<a name="0x3_iota_system_validator_voting_powers"></a>

## Function `validator_voting_powers`

Returns the voting power of the active validators, values are voting power in the scale of 10000.


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_validator_voting_powers">validator_voting_powers</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): <a href="../iota-framework/vec_map.md#0x2_vec_map_VecMap">vec_map::VecMap</a>&lt;<b>address</b>, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="iota_system.md#0x3_iota_system_validator_voting_powers">validator_voting_powers</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): VecMap&lt;<b>address</b>, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>&gt; {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state">load_system_state</a>(wrapper);
    <a href="iota_system_state_inner.md#0x3_iota_system_state_inner_active_validator_voting_powers">iota_system_state_inner::active_validator_voting_powers</a>(self)
}
</code></pre>



</details>

<a name="0x3_iota_system_get_total_iota_supply"></a>

## Function `get_total_iota_supply`

Returns the total iota supply.


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_get_total_iota_supply">get_total_iota_supply</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="iota_system.md#0x3_iota_system_get_total_iota_supply">get_total_iota_supply</a>(wrapper: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">IotaSystemState</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    <b>let</b> self = <a href="iota_system.md#0x3_iota_system_load_system_state">load_system_state</a>(wrapper);
    self.<a href="iota_system.md#0x3_iota_system_get_total_iota_supply">get_total_iota_supply</a>()
}
</code></pre>



</details>
