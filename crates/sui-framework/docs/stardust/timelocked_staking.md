---
title: Module `0x107a::timelocked_staking`
---



-  [Constants](#@Constants_0)
-  [Function `request_add_stake`](#0x107a_timelocked_staking_request_add_stake)
-  [Function `request_add_stake_non_entry`](#0x107a_timelocked_staking_request_add_stake_non_entry)
-  [Function `request_add_stake_mul_bal`](#0x107a_timelocked_staking_request_add_stake_mul_bal)
-  [Function `request_add_stake_mul_bal_non_entry`](#0x107a_timelocked_staking_request_add_stake_mul_bal_non_entry)
-  [Function `request_withdraw_stake`](#0x107a_timelocked_staking_request_withdraw_stake)
-  [Function `request_withdraw_stake_non_entry`](#0x107a_timelocked_staking_request_withdraw_stake_non_entry)


<pre><code><b>use</b> <a href="timelock.md#0x107a_timelock">0x107a::timelock</a>;
<b>use</b> <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">0x107a::timelocked_staked_sui</a>;
<b>use</b> <a href="../sui-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../sui-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../sui-framework/sui.md#0x2_sui">0x2::sui</a>;
<b>use</b> <a href="../sui-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../sui-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../sui-system/staking_pool.md#0x3_staking_pool">0x3::staking_pool</a>;
<b>use</b> <a href="../sui-system/sui_system.md#0x3_sui_system">0x3::sui_system</a>;
</code></pre>



<a name="@Constants_0"></a>

## Constants


<a name="0x107a_timelocked_staking_ETimeLockShouldNotBeExpired"></a>

For when trying to stake an expired time-locked balance.


<pre><code><b>const</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_ETimeLockShouldNotBeExpired">ETimeLockShouldNotBeExpired</a>: u64 = 0;
</code></pre>



<a name="0x107a_timelocked_staking_request_add_stake"></a>

## Function `request_add_stake`

Add a time-locked stake to a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake">request_add_stake</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> <a href="../sui-system/sui_system.md#0x3_sui_system_SuiSystemState">sui_system::SuiSystemState</a>, <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>: <a href="timelock.md#0x107a_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../sui-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../sui-framework/sui.md#0x2_sui_SUI">sui::SUI</a>&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake">request_add_stake</a>(
    <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> SuiSystemState,
    <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>: TimeLock&lt;Balance&lt;SUI&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    // Stake the time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a> = <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>, <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>, validator_address, ctx);

    // Transfer the receipt <b>to</b> the sender.
    <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_transfer">timelocked_staked_sui::transfer</a>(<a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>, ctx.sender());
}
</code></pre>



</details>

<a name="0x107a_timelocked_staking_request_add_stake_non_entry"></a>

## Function `request_add_stake_non_entry`

The non-entry version of <code>request_add_stake</code>, which returns the time-locked staked SUI instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> <a href="../sui-system/sui_system.md#0x3_sui_system_SuiSystemState">sui_system::SuiSystemState</a>, <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>: <a href="timelock.md#0x107a_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../sui-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../sui-framework/sui.md#0x2_sui_SUI">sui::SUI</a>&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(
    <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> SuiSystemState,
    <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>: TimeLock&lt;Balance&lt;SUI&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) : TimelockedStakedSui {
    // Check the preconditions.
    <b>assert</b>!(<a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>.is_locked(ctx), <a href="timelocked_staking.md#0x107a_timelocked_staking_ETimeLockShouldNotBeExpired">ETimeLockShouldNotBeExpired</a>);

    // Unpack the time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (<a href="../sui-framework/balance.md#0x2_balance">balance</a>, expiration_timestamp_ms) = <a href="timelock.md#0x107a_timelock_unpack">timelock::unpack</a>(<a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>);

    // Stake the time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> staked_sui = <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>.<a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(
        <a href="../sui-framework/balance.md#0x2_balance">balance</a>.into_coin(ctx),
        validator_address,
        ctx,
    );

    // Create and <b>return</b> a receipt.
    <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_create">timelocked_staked_sui::create</a>(
        staked_sui,
        expiration_timestamp_ms,
        ctx
    )
}
</code></pre>



</details>

<a name="0x107a_timelocked_staking_request_add_stake_mul_bal"></a>

## Function `request_add_stake_mul_bal`

Add a time-locked stake to a validator's staking pool using multiple time-locked balances.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_mul_bal">request_add_stake_mul_bal</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> <a href="../sui-system/sui_system.md#0x3_sui_system_SuiSystemState">sui_system::SuiSystemState</a>, timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelock.md#0x107a_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../sui-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../sui-framework/sui.md#0x2_sui_SUI">sui::SUI</a>&gt;&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_mul_bal">request_add_stake_mul_bal</a>(
    <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> SuiSystemState,
    timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;SUI&gt;&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) {
    // Stake the time-locked balances.
    <b>let</b> <b>mut</b> receipts = <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>, timelocked_balances, validator_address, ctx);

    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, receipts.length());

    // Send all the receipts <b>to</b> the sender.
    <b>while</b> (i &lt; len) {
        // Take a receipt.
        <b>let</b> receipt = receipts.pop_back();

        // Transfer the receipt <b>to</b> the sender.
        <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_transfer">timelocked_staked_sui::transfer</a>(receipt, ctx.sender());

        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(receipts)
}
</code></pre>



</details>

<a name="0x107a_timelocked_staking_request_add_stake_mul_bal_non_entry"></a>

## Function `request_add_stake_mul_bal_non_entry`

The non-entry version of <code>request_add_stake_mul_bal</code>,
which returns a list of the time-locked staked SUIs instead of transferring them to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> <a href="../sui-system/sui_system.md#0x3_sui_system_SuiSystemState">sui_system::SuiSystemState</a>, timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelock.md#0x107a_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../sui-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../sui-framework/sui.md#0x2_sui_SUI">sui::SUI</a>&gt;&gt;&gt;, validator_address: <b>address</b>, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_mul_bal_non_entry">request_add_stake_mul_bal_non_entry</a>(
    <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> SuiSystemState,
    <b>mut</b> timelocked_balances: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;SUI&gt;&gt;&gt;,
    validator_address: <b>address</b>,
    ctx: &<b>mut</b> TxContext,
) : <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimelockedStakedSui&gt; {
    // Create a <a href="../move-stdlib/vector.md#0x1_vector">vector</a> <b>to</b> store the results.
    <b>let</b> <b>mut</b> result = <a href="../move-stdlib/vector.md#0x1_vector">vector</a>[];

    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, timelocked_balances.length());

    // Stake all the time-locked balances.
    <b>while</b> (i &lt; len) {
        // Take a time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
        <b>let</b> <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a> = timelocked_balances.pop_back();

        // Stake the time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
        <b>let</b> <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a> = <a href="timelocked_staking.md#0x107a_timelocked_staking_request_add_stake_non_entry">request_add_stake_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>, <a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>, validator_address, ctx);

        // Store the created receipt.
        result.push_back(<a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>);

        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(timelocked_balances);

    result
}
</code></pre>



</details>

<a name="0x107a_timelocked_staking_request_withdraw_stake"></a>

## Function `request_withdraw_stake`

Withdraw a time-locked stake from a validator's staking pool.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_withdraw_stake">request_withdraw_stake</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> <a href="../sui-system/sui_system.md#0x3_sui_system_SuiSystemState">sui_system::SuiSystemState</a>, <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>: <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_withdraw_stake">request_withdraw_stake</a>(
    <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> SuiSystemState,
    <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>: TimelockedStakedSui,
    ctx: &<b>mut</b> TxContext,
) {
    // Withdraw the time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (<a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>, reward) = <a href="timelocked_staking.md#0x107a_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>, <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>, ctx);

    // Transfer the withdrawn time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a> <b>to</b> the sender.
    <a href="timelock.md#0x107a_timelock_transfer">timelock::transfer</a>(<a href="timelocked_balance.md#0x107a_timelocked_balance">timelocked_balance</a>, ctx.sender());

    // Send coins only <b>if</b> the reward is not zero.
    <b>if</b> (reward.value() &gt; 0) {
        <a href="../sui-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(reward.into_coin(ctx), ctx.sender());
    }
    <b>else</b> {
        <a href="../sui-framework/balance.md#0x2_balance_destroy_zero">balance::destroy_zero</a>(reward);
    }
}
</code></pre>



</details>

<a name="0x107a_timelocked_staking_request_withdraw_stake_non_entry"></a>

## Function `request_withdraw_stake_non_entry`

Non-entry version of <code>request_withdraw_stake</code> that returns the withdrawn time-locked SUI and reward
instead of transferring it to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(<a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> <a href="../sui-system/sui_system.md#0x3_sui_system_SuiSystemState">sui_system::SuiSystemState</a>, <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>: <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): (<a href="timelock.md#0x107a_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../sui-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../sui-framework/sui.md#0x2_sui_SUI">sui::SUI</a>&gt;&gt;, <a href="../sui-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../sui-framework/sui.md#0x2_sui_SUI">sui::SUI</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staking.md#0x107a_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(
    <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>: &<b>mut</b> SuiSystemState,
    <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>: TimelockedStakedSui,
    ctx: &<b>mut</b> TxContext,
) : (TimeLock&lt;Balance&lt;SUI&gt;&gt;, Balance&lt;SUI&gt;) {
    // Unpack the `TimelockedStakedSui` instance.
    <b>let</b> (staked_sui, expiration_timestamp_ms) = <a href="timelocked_staked_sui.md#0x107a_timelocked_staked_sui">timelocked_staked_sui</a>.unpack();

    // Store the original stake amount.
    <b>let</b> principal = staked_sui.staked_sui_amount();

    // Withdraw the <a href="../sui-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> <b>mut</b> withdraw_stake = <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a>.<a href="timelocked_staking.md#0x107a_timelocked_staking_request_withdraw_stake_non_entry">request_withdraw_stake_non_entry</a>(staked_sui, ctx);

    // The <a href="../sui-system/sui_system.md#0x3_sui_system">sui_system</a> withdraw functions <b>return</b> a <a href="../sui-framework/balance.md#0x2_balance">balance</a> that consists of the original staked amount plus the reward amount;
    // In here, it splits the original staked <a href="../sui-framework/balance.md#0x2_balance">balance</a> <b>to</b> <a href="timelock.md#0x107a_timelock">timelock</a> it again.
    <b>let</b> principal = withdraw_stake.split(principal);

    // Pack and <b>return</b> a time-locked <a href="../sui-framework/balance.md#0x2_balance">balance</a>, and the reward.
    (<a href="timelock.md#0x107a_timelock_pack">timelock::pack</a>(principal, expiration_timestamp_ms, ctx), withdraw_stake)
}
</code></pre>



</details>
