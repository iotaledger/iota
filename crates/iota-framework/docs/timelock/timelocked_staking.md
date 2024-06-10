---
title: Module `0x10cf::timelocked_staking`
---



-  [Constants](#@Constants_0)
-  [Function `request_add_stake`](#0x10cf_timelocked_staking_request_add_stake)
-  [Function `request_add_stake_non_entry`](#0x10cf_timelocked_staking_request_add_stake_non_entry)
-  [Function `request_add_stake_mul_bal`](#0x10cf_timelocked_staking_request_add_stake_mul_bal)
-  [Function `request_add_stake_mul_bal_non_entry`](#0x10cf_timelocked_staking_request_add_stake_mul_bal_non_entry)
-  [Function `request_withdraw_stake`](#0x10cf_timelocked_staking_request_withdraw_stake)
-  [Function `request_withdraw_stake_non_entry`](#0x10cf_timelocked_staking_request_withdraw_stake_non_entry)


<pre><code><b>use</b> <a href="timelock.md#0x10cf_timelock">0x10cf::timelock</a>;
<b>use</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">0x10cf::timelocked_staked_iota</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/iota.md#0x2_iota">0x2::iota</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-system/iota_system.md#0x3_iota_system">0x3::iota_system</a>;
<b>use</b> <a href="../iota-system/staking_pool.md#0x3_staking_pool">0x3::staking_pool</a>;
</code></pre>



<a name="@Constants_0"></a>

## Constants


<a name="0x10cf_timelocked_staking_ETimeLockShouldNotBeExpired"></a>

For when trying to stake an expired time-locked balance.


<pre><code><b>const</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_ETimeLockShouldNotBeExpired">ETimeLockShouldNotBeExpired</a>: u64 = 0;
</code></pre>



<a name="0x10cf_timelocked_staking_request_add_stake"></a>

## Function `request_add_stake`

Add a time-locked stake to a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake">request_add_stake</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="../iota-system/iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>: <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake">request_add_stake</a>(
    <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>: TimeLock&lt;Balance&lt;IOTA&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    // Stake the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a> = <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>, <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>, validator_address, ctx);

    // Transfer the receipt <b>to</b> the sender.
    <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_transfer">timelocked_staked_iota::transfer</a>(<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>, ctx.sender());
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staking_request_add_stake_non_entry"></a>

## Function `request_add_stake_non_entry`

The non-entry version of <code>request_add_stake</code>, which returns the time-locked staked IOTA instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="../iota-system/iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>: <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(
    <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>: TimeLock&lt;Balance&lt;IOTA&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) : TimelockedStakedIota {
    // Check the preconditions.
    <b>assert</b>!(<a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>.is_locked(ctx), <a href="timelocked_staking.md#0x10cf_timelocked_staking_ETimeLockShouldNotBeExpired">ETimeLockShouldNotBeExpired</a>);

    // Unpack the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (<a href="../iota-framework/balance.md#0x2_balance">balance</a>, expiration_timestamp_ms, label) = <a href="timelock.md#0x10cf_timelock_unpack">timelock::unpack</a>(<a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>);

    // Stake the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> staked_iota = <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>.<a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(
        <a href="../iota-framework/balance.md#0x2_balance">balance</a>.into_coin(ctx),
        validator_address,
        ctx,
    );

    // Create and <b>return</b> a receipt.
    <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_create">timelocked_staked_iota::create</a>(
        staked_iota,
        expiration_timestamp_ms,
        label,
        ctx,
    )
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staking_request_add_stake_mul_bal"></a>

## Function `request_add_stake_mul_bal`

Add a time-locked stake to a validator's staking pool using multiple time-locked balances.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_mul_bal">request_add_stake_mul_bal</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="../iota-system/iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_mul_bal">request_add_stake_mul_bal</a>(
    <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;IOTA&gt;&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    // Stake the time-locked balances.
    <b>let</b> <b>mut</b> receipts = <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>, timelocked_balances, validator_address, ctx);

    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, receipts.length());

    // Send all the receipts <b>to</b> the sender.
    <b>while</b> (i &lt; len) {
        // Take a receipt.
        <b>let</b> receipt = receipts.pop_back();

        // Transfer the receipt <b>to</b> the sender.
        <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_transfer">timelocked_staked_iota::transfer</a>(receipt, ctx.sender());

        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(receipts)
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staking_request_add_stake_mul_bal_non_entry"></a>

## Function `request_add_stake_mul_bal_non_entry`

The non-entry version of <code>request_add_stake_mul_bal</code>,
which returns a list of the time-locked staked IOTAs instead of transferring them to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="../iota-system/iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(
    <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    <b>mut</b> timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;IOTA&gt;&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) : <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimelockedStakedIota&gt; {
    // Create a <a href="../move-stdlib/vector.md#0x1_vector">vector</a> <b>to</b> store the results.
    <b>let</b> <b>mut</b> result = <a href="../move-stdlib/vector.md#0x1_vector">vector</a>[];

    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, timelocked_balances.length());

    // Stake all the time-locked balances.
    <b>while</b> (i &lt; len) {
        // Take a time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
        <b>let</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a> = timelocked_balances.pop_back();

        // Stake the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
        <b>let</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a> = <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>, <a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>, validator_address, ctx);

        // Store the created receipt.
        result.push_back(<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>);

        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(timelocked_balances);

    result
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staking_request_withdraw_stake"></a>

## Function `request_withdraw_stake`

Withdraw a time-locked stake from a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_withdraw_stake">request_withdraw_stake</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="../iota-system/iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_withdraw_stake">request_withdraw_stake</a>(
    <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>: TimelockedStakedIota,
    ctx: &<b>mut</b> TxContext,
) {
    // Withdraw the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (<a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>, reward) = <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>, <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>, ctx);

    // Transfer the withdrawn time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a> <b>to</b> the sender.
    <a href="timelock.md#0x10cf_timelock_transfer">timelock::transfer</a>(<a href="timelocked_balance.md#0x10cf_timelocked_balance">timelocked_balance</a>, ctx.sender());

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

<a name="0x10cf_timelocked_staking_request_withdraw_stake_non_entry"></a>

## Function `request_withdraw_stake_non_entry`

Non-entry version of <code>request_withdraw_stake</code> that returns the withdrawn time-locked IOTA and reward
instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(<a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> <a href="../iota-system/iota_system.md#0x3_iota_system_IotaSystemState">iota_system::IotaSystemState</a>, <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): (<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;&gt;, <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x10cf_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(
    <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>: &<b>mut</b> IotaSystemState,
    <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>: TimelockedStakedIota,
    ctx: &<b>mut</b> TxContext,
) : (TimeLock&lt;Balance&lt;IOTA&gt;&gt;, Balance&lt;IOTA&gt;) {
    // Unpack the `TimelockedStakedIota` instance.
    <b>let</b> (staked_iota, expiration_timestamp_ms, label) = <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota">timelocked_staked_iota</a>.unpack();

    // Store the original stake amount.
    <b>let</b> principal = staked_iota.staked_iota_amount();

    // Withdraw the <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> <b>mut</b> withdraw_stake = <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a>.<a href="timelocked_staking.md#0x10cf_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(staked_iota, ctx);

    // The <a href="../iota-system/iota_system.md#0x3_iota_system">iota_system</a> withdraw functions <b>return</b> a <a href="../iota-framework/balance.md#0x2_balance">balance</a> that consists of the original staked amount plus the reward amount;
    // In here, it splits the original staked <a href="../iota-framework/balance.md#0x2_balance">balance</a> <b>to</b> <a href="timelock.md#0x10cf_timelock">timelock</a> it again.
    <b>let</b> principal = withdraw_stake.split(principal);

    // Pack and <b>return</b> a time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>, and the reward.
    (<a href="timelock.md#0x10cf_timelock_pack">timelock::pack</a>(principal, expiration_timestamp_ms, label, ctx), withdraw_stake)
}
</code></pre>



</details>
