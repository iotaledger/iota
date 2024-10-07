---
title: Module `0x3::timelocked_staking`
---



-  [Resource `TimelockedStakedIota`](#0x3_timelocked_staking_TimelockedStakedIota)
-  [Constants](#@Constants_0)
-  [Function `request_add_stake`](#0x3_timelocked_staking_request_add_stake)
-  [Function `request_add_stake_mul_bal`](#0x3_timelocked_staking_request_add_stake_mul_bal)
-  [Function `request_withdraw_stake`](#0x3_timelocked_staking_request_withdraw_stake)
-  [Function `request_add_stake_non_entry`](#0x3_timelocked_staking_request_add_stake_non_entry)
-  [Function `request_add_stake_mul_bal_non_entry`](#0x3_timelocked_staking_request_add_stake_mul_bal_non_entry)
-  [Function `request_withdraw_stake_non_entry`](#0x3_timelocked_staking_request_withdraw_stake_non_entry)
-  [Function `split`](#0x3_timelocked_staking_split)
-  [Function `split_staked_iota`](#0x3_timelocked_staking_split_staked_iota)
-  [Function `join_staked_iota`](#0x3_timelocked_staking_join_staked_iota)
-  [Function `transfer_to_sender`](#0x3_timelocked_staking_transfer_to_sender)
-  [Function `transfer_to_sender_multiple`](#0x3_timelocked_staking_transfer_to_sender_multiple)
-  [Function `is_equal_staking_metadata`](#0x3_timelocked_staking_is_equal_staking_metadata)
-  [Function `pool_id`](#0x3_timelocked_staking_pool_id)
-  [Function `staked_iota_amount`](#0x3_timelocked_staking_staked_iota_amount)
-  [Function `stake_activation_epoch`](#0x3_timelocked_staking_stake_activation_epoch)
-  [Function `expiration_timestamp_ms`](#0x3_timelocked_staking_expiration_timestamp_ms)
-  [Function `label`](#0x3_timelocked_staking_label)
-  [Function `is_labeled_with`](#0x3_timelocked_staking_is_labeled_with)
-  [Function `unpack`](#0x3_timelocked_staking_unpack)
-  [Function `transfer`](#0x3_timelocked_staking_transfer)
-  [Function `transfer_multiple`](#0x3_timelocked_staking_transfer_multiple)
-  [Function `request_add_stake_at_genesis`](#0x3_timelocked_staking_request_add_stake_at_genesis)


<pre><code><b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../move-stdlib/vector.md#0x1_vector">0x1::vector</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/iota.md#0x2_iota">0x2::iota</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/timelock.md#0x2_timelock">0x2::timelock</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="iota_system.md#0x3_iota_system">0x3::iota_system</a>;
<b>use</b> <a href="staking_pool.md#0x3_staking_pool">0x3::staking_pool</a>;
<b>use</b> <a href="validator.md#0x3_validator">0x3::validator</a>;
</code></pre>



<a name="0x3_timelocked_staking_TimelockedStakedIota"></a>

## Resource `TimelockedStakedIota`

A self-custodial object holding the timelocked staked IOTA tokens.


<pre><code><b>struct</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> <b>has</b> key
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
<code>staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a></code>
</dt>
<dd>
 A self-custodial object holding the staked IOTA tokens.
</dd>
<dt>
<code>expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 This is the epoch time stamp of when the lock expires.
</dd>
<dt>
<code>label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;</code>
</dt>
<dd>
 Timelock related label.
</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x3_timelocked_staking_EIncompatibleTimelockedStakedIota"></a>

Incompatible objects when joining TimelockedStakedIota


<pre><code><b>const</b> <a href="timelocked_staking.md#0x3_timelocked_staking_EIncompatibleTimelockedStakedIota">EIncompatibleTimelockedStakedIota</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1;
</code></pre>



<a name="0x3_timelocked_staking_ETimeLockShouldNotBeExpired"></a>

For when trying to stake an expired time-locked balance.


<pre><code><b>const</b> <a href="timelocked_staking.md#0x3_timelocked_staking_ETimeLockShouldNotBeExpired">ETimeLockShouldNotBeExpired</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x3_timelocked_staking_request_add_stake"></a>

## Function `request_add_stake`

Add a time-locked stake to a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake">request_add_stake</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_balance: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake">request_add_stake</a>(
    <a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    timelocked_balance: TimeLock&lt;Balance&lt;IOTA&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    // Stake the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> timelocked_staked_iota = <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>, timelocked_balance, validator_address, ctx);

    // Transfer the receipt <b>to</b> the sender.
    timelocked_staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender">transfer_to_sender</a>(ctx);
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_request_add_stake_mul_bal"></a>

## Function `request_add_stake_mul_bal`

Add a time-locked stake to a validator's staking pool using multiple time-locked balances.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_mul_bal">request_add_stake_mul_bal</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_mul_bal">request_add_stake_mul_bal</a>(
    <a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;IOTA&gt;&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    // Stake the time-locked balances.
    <b>let</b> <b>mut</b> receipts = <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>, timelocked_balances, validator_address, ctx);

    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, receipts.length());

    // Send all the receipts <b>to</b> the sender.
    <b>while</b> (i &lt; len) {
        // Take a receipt.
        <b>let</b> receipt = receipts.pop_back();

        // Transfer the receipt <b>to</b> the sender.
        receipt.<a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender">transfer_to_sender</a>(ctx);

        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(receipts)
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_request_withdraw_stake"></a>

## Function `request_withdraw_stake`

Withdraw a time-locked stake from a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_withdraw_stake">request_withdraw_stake</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_staked_iota: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_withdraw_stake">request_withdraw_stake</a>(
    <a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    timelocked_staked_iota: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>,
    ctx: &<b>mut</b> TxContext,
) {
    // Withdraw the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (timelocked_balance, reward) = <a href="timelocked_staking.md#0x3_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>, timelocked_staked_iota, ctx);

    // Transfer the withdrawn time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a> <b>to</b> the sender.
   timelocked_balance.<a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender">transfer_to_sender</a>(ctx);

    // Send coins only <b>if</b> the reward is not zero.
    <b>if</b> (reward.value() &gt; 0) {
        <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(reward.into_coin(ctx), ctx.sender());
    }
    <b>else</b> {
        <a href="../iota-framework/balance.md#0x2_balance_destroy_zero">balance::destroy_zero</a>(reward);
    }
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_request_add_stake_non_entry"></a>

## Function `request_add_stake_non_entry`

The non-entry version of <code>request_add_stake</code>, which returns the time-locked staked IOTA instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_balance: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(
    <a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    timelocked_balance: TimeLock&lt;Balance&lt;IOTA&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) : <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
    // Check the preconditions.
    <b>assert</b>!(timelocked_balance.is_locked(ctx), <a href="timelocked_staking.md#0x3_timelocked_staking_ETimeLockShouldNotBeExpired">ETimeLockShouldNotBeExpired</a>);

    // Unpack the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> sys_timelock_cap = <a href="iota_system.md#0x3_iota_system">iota_system</a>.load_system_timelock_cap();
    <b>let</b> (<a href="../iota-framework/balance.md#0x2_balance">balance</a>, expiration_timestamp_ms, label) = <a href="../iota-framework/timelock.md#0x2_timelock_system_unpack">timelock::system_unpack</a>(sys_timelock_cap, timelocked_balance);

    // Stake the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> staked_iota = <a href="iota_system.md#0x3_iota_system">iota_system</a>.<a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(
        <a href="../iota-framework/balance.md#0x2_balance">balance</a>.into_coin(ctx),
        validator_address,
        ctx,
    );

    // Create and <b>return</b> a receipt.
    <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_iota,
        expiration_timestamp_ms,
        label,
    }
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_request_add_stake_mul_bal_non_entry"></a>

## Function `request_add_stake_mul_bal_non_entry`

The non-entry version of <code>request_add_stake_mul_bal</code>,
which returns a list of the time-locked staked IOTAs instead of transferring them to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(
    <a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    <b>mut</b> timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;IOTA&gt;&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) : <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>&gt; {
    // Create a <a href="../move-stdlib/vector.md#0x1_vector">vector</a> <b>to</b> store the results.
    <b>let</b> <b>mut</b> result = <a href="../move-stdlib/vector.md#0x1_vector">vector</a>[];

    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, timelocked_balances.length());

    // Stake all the time-locked balances.
    <b>while</b> (i &lt; len) {
        // Take a time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
        <b>let</b> timelocked_balance = timelocked_balances.pop_back();

        // Stake the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
        <b>let</b> timelocked_staked_iota = <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>, timelocked_balance, validator_address, ctx);

        // Store the created receipt.
        result.push_back(timelocked_staked_iota);

        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(timelocked_balances);

    result
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_request_withdraw_stake_non_entry"></a>

## Function `request_withdraw_stake_non_entry`

Non-entry version of <code>request_withdraw_stake</code> that returns the withdrawn time-locked IOTA and reward
instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(<a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_staked_iota: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): (<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(
    <a href="iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    timelocked_staked_iota: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>,
    ctx: &<b>mut</b> TxContext,
) : (TimeLock&lt;Balance&lt;IOTA&gt;&gt;, Balance&lt;IOTA&gt;) {
    // Unpack the `<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>` instance.
    <b>let</b> (staked_iota, expiration_timestamp_ms, label) = timelocked_staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_unpack">unpack</a>();

    // Store the original stake amount.
    <b>let</b> principal = staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_staked_iota_amount">staked_iota_amount</a>();

    // Withdraw the <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> <b>mut</b> withdraw_stake = <a href="iota_system.md#0x3_iota_system">iota_system</a>.<a href="timelocked_staking.md#0x3_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(staked_iota, ctx);

    // The <a href="iota_system.md#0x3_iota_system">iota_system</a> withdraw functions <b>return</b> a <a href="../iota-framework/balance.md#0x2_balance">balance</a> that consists of the original staked amount plus the reward amount;
    // In here, it splits the original staked <a href="../iota-framework/balance.md#0x2_balance">balance</a> <b>to</b> <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a> it again.
    <b>let</b> principal = withdraw_stake.<a href="timelocked_staking.md#0x3_timelocked_staking_split">split</a>(principal);

    // Pack and <b>return</b> a time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>, and the reward.
    <b>let</b> sys_timelock_cap = <a href="iota_system.md#0x3_iota_system">iota_system</a>.load_system_timelock_cap();
    (<a href="../iota-framework/timelock.md#0x2_timelock_system_pack">timelock::system_pack</a>(sys_timelock_cap, principal, expiration_timestamp_ms, label, ctx), withdraw_stake)
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_split"></a>

## Function `split`

Split <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code> into two parts, one with principal <code>split_amount</code>,
and the remaining principal is left in <code>self</code>.
All the other parameters of the <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code> like <code>stake_activation_epoch</code> or <code>pool_id</code> remain the same.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_split">split</a>(self: &<b>mut</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_split">split</a>(self: &<b>mut</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext): <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
    <b>let</b> split_stake = self.staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_split">split</a>(split_amount, ctx);

    <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_iota: split_stake,
        expiration_timestamp_ms: self.expiration_timestamp_ms,
        label: self.label,
    }
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_split_staked_iota"></a>

## Function `split_staked_iota`

Split the given <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code> to the two parts, one with principal <code>split_amount</code>,
transfer the newly split part to the sender address.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_split_staked_iota">split_staked_iota</a>(stake: &<b>mut</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_split_staked_iota">split_staked_iota</a>(stake: &<b>mut</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext) {
    <a href="timelocked_staking.md#0x3_timelocked_staking_split">split</a>(stake, split_amount, ctx).<a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender">transfer_to_sender</a>(ctx);
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_join_staked_iota"></a>

## Function `join_staked_iota`

Consume the staked iota <code>other</code> and add its value to <code>self</code>.
Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_join_staked_iota">join_staked_iota</a>(self: &<b>mut</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, other: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_join_staked_iota">join_staked_iota</a>(self: &<b>mut</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>, other: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>) {
    <b>assert</b>!(self.<a href="timelocked_staking.md#0x3_timelocked_staking_is_equal_staking_metadata">is_equal_staking_metadata</a>(&other), <a href="timelocked_staking.md#0x3_timelocked_staking_EIncompatibleTimelockedStakedIota">EIncompatibleTimelockedStakedIota</a>);

    <b>let</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
        id,
        staked_iota,
        expiration_timestamp_ms: _,
        label: _,
    } = other;

    id.delete();

    self.staked_iota.join(staked_iota);
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_transfer_to_sender"></a>

## Function `transfer_to_sender`

A utility function to transfer a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender">transfer_to_sender</a>(stake: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender">transfer_to_sender</a>(stake: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>, ctx: &TxContext) {
    <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(stake, ctx.sender())
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_transfer_to_sender_multiple"></a>

## Function `transfer_to_sender_multiple`

A utility function to transfer multiple <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender_multiple">transfer_to_sender_multiple</a>(stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_to_sender_multiple">transfer_to_sender_multiple</a>(stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>&gt;, ctx: &TxContext) {
    <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_multiple">transfer_multiple</a>(stakes, ctx.sender())
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_is_equal_staking_metadata"></a>

## Function `is_equal_staking_metadata`

A utility function that returns true if all the staking parameters
of the staked iota except the principal are identical


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, other: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>, other: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): bool {
    self.staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_is_equal_staking_metadata">is_equal_staking_metadata</a>(&other.staked_iota) &&
    (self.expiration_timestamp_ms == other.expiration_timestamp_ms) &&
    (self.<a href="timelocked_staking.md#0x3_timelocked_staking_label">label</a>() == other.<a href="timelocked_staking.md#0x3_timelocked_staking_label">label</a>())
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_pool_id"></a>

## Function `pool_id`

Function to get the pool id of a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_pool_id">pool_id</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): <a href="../iota-framework/object.md#0x2_object_ID">object::ID</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_pool_id">pool_id</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): ID { self.staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_pool_id">pool_id</a>() }
</code></pre>



</details>

<a name="0x3_timelocked_staking_staked_iota_amount"></a>

## Function `staked_iota_amount`

Function to get the staked iota amount of a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_staked_iota_amount">staked_iota_amount</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_staked_iota_amount">staked_iota_amount</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> { self.staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_staked_iota_amount">staked_iota_amount</a>() }
</code></pre>



</details>

<a name="0x3_timelocked_staking_stake_activation_epoch"></a>

## Function `stake_activation_epoch`

Function to get the stake activation epoch of a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_stake_activation_epoch">stake_activation_epoch</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_stake_activation_epoch">stake_activation_epoch</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    self.staked_iota.<a href="timelocked_staking.md#0x3_timelocked_staking_stake_activation_epoch">stake_activation_epoch</a>()
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_expiration_timestamp_ms"></a>

## Function `expiration_timestamp_ms`

Function to get the expiration timestamp of a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_expiration_timestamp_ms">expiration_timestamp_ms</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_expiration_timestamp_ms">expiration_timestamp_ms</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    self.expiration_timestamp_ms
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_label"></a>

## Function `label`

Function to get the label of a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_label">label</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_label">label</a>(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): Option&lt;String&gt; {
    self.label
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_is_labeled_with"></a>

## Function `is_labeled_with`

Check if a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code> is labeled with the type <code>L</code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_is_labeled_with">is_labeled_with</a>&lt;L&gt;(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_is_labeled_with">is_labeled_with</a>&lt;L&gt;(self: &<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): bool {
    <b>if</b> (self.label.is_some()) {
        self.label.borrow() == <a href="../iota-framework/timelock.md#0x2_timelock_type_name">timelock::type_name</a>&lt;L&gt;()
    }
    <b>else</b> {
        <b>false</b>
    }
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_unpack"></a>

## Function `unpack`

A utility function to destroy a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_unpack">unpack</a>(self: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>): (<a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_unpack">unpack</a>(self: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>): (StakedIota, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, Option&lt;String&gt;) {
    <b>let</b> <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
        id,
        staked_iota,
        expiration_timestamp_ms,
        label,
    } = self;

    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);

    (staked_iota, expiration_timestamp_ms, label)
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_transfer"></a>

## Function `transfer`

A utility function to transfer a <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code> to a receiver.


<pre><code><b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(stake: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>, receiver: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(stake: <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>, receiver: <b>address</b>) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(stake, receiver);
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_transfer_multiple"></a>

## Function `transfer_multiple`

A utility function to transfer a vector of <code><a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a></code> to a receiver.


<pre><code><b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_multiple">transfer_multiple</a>(stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">timelocked_staking::TimelockedStakedIota</a>&gt;, receiver: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_transfer_multiple">transfer_multiple</a>(<b>mut</b> stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a>&gt;, receiver: <b>address</b>) {
    // Transfer all the time-locked stakes <b>to</b> the recipient.
    <b>while</b> (!stakes.is_empty()) {
       <b>let</b> stake = stakes.pop_back();
       <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(stake, receiver);
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(stakes);
}
</code></pre>



</details>

<a name="0x3_timelocked_staking_request_add_stake_at_genesis"></a>

## Function `request_add_stake_at_genesis`

Request to add timelocked stake to the validator's staking pool at genesis


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_at_genesis">request_add_stake_at_genesis</a>(<a href="validator.md#0x3_validator">validator</a>: &<b>mut</b> <a href="validator.md#0x3_validator_Validator">validator::Validator</a>, stake: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, staker_address: <b>address</b>, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelocked_staking.md#0x3_timelocked_staking_request_add_stake_at_genesis">request_add_stake_at_genesis</a>(
    <a href="validator.md#0x3_validator">validator</a>: &<b>mut</b> Validator,
    stake: Balance&lt;IOTA&gt;,
    staker_address: <b>address</b>,
    expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    label: Option&lt;String&gt;,
    ctx: &<b>mut</b> TxContext,
) {
    <b>let</b> staked_iota = <a href="validator.md#0x3_validator">validator</a>.request_add_stake_at_genesis_with_receipt(stake, ctx);
    <b>let</b> timelocked_staked_iota = <a href="timelocked_staking.md#0x3_timelocked_staking_TimelockedStakedIota">TimelockedStakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_iota,
        expiration_timestamp_ms,
        label,
    };
    <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(timelocked_staked_iota, staker_address);
}
</code></pre>



</details>
