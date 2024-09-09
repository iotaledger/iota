---
title: Module `0x3::staking_pool`
---



-  [Resource `StakingPool`](#0x3_staking_pool_StakingPool)
-  [Struct `PoolTokenExchangeRate`](#0x3_staking_pool_PoolTokenExchangeRate)
-  [Resource `StakedIota`](#0x3_staking_pool_StakedIota)
-  [Constants](#@Constants_0)
-  [Function `new`](#0x3_staking_pool_new)
-  [Function `request_add_stake`](#0x3_staking_pool_request_add_stake)
-  [Function `request_withdraw_stake`](#0x3_staking_pool_request_withdraw_stake)
-  [Function `withdraw_from_principal`](#0x3_staking_pool_withdraw_from_principal)
-  [Function `unwrap_staked_iota`](#0x3_staking_pool_unwrap_staked_iota)
-  [Function `deposit_rewards`](#0x3_staking_pool_deposit_rewards)
-  [Function `process_pending_stakes_and_withdraws`](#0x3_staking_pool_process_pending_stakes_and_withdraws)
-  [Function `process_pending_stake_withdraw`](#0x3_staking_pool_process_pending_stake_withdraw)
-  [Function `process_pending_stake`](#0x3_staking_pool_process_pending_stake)
-  [Function `withdraw_rewards`](#0x3_staking_pool_withdraw_rewards)
-  [Function `activate_staking_pool`](#0x3_staking_pool_activate_staking_pool)
-  [Function `deactivate_staking_pool`](#0x3_staking_pool_deactivate_staking_pool)
-  [Function `iota_balance`](#0x3_staking_pool_iota_balance)
-  [Function `pool_id`](#0x3_staking_pool_pool_id)
-  [Function `staked_iota_amount`](#0x3_staking_pool_staked_iota_amount)
-  [Function `stake_activation_epoch`](#0x3_staking_pool_stake_activation_epoch)
-  [Function `is_preactive`](#0x3_staking_pool_is_preactive)
-  [Function `is_inactive`](#0x3_staking_pool_is_inactive)
-  [Function `split`](#0x3_staking_pool_split)
-  [Function `split_staked_iota`](#0x3_staking_pool_split_staked_iota)
-  [Function `join_staked_iota`](#0x3_staking_pool_join_staked_iota)
-  [Function `is_equal_staking_metadata`](#0x3_staking_pool_is_equal_staking_metadata)
-  [Function `pool_token_exchange_rate_at_epoch`](#0x3_staking_pool_pool_token_exchange_rate_at_epoch)
-  [Function `pending_stake_amount`](#0x3_staking_pool_pending_stake_amount)
-  [Function `pending_stake_withdraw_amount`](#0x3_staking_pool_pending_stake_withdraw_amount)
-  [Function `exchange_rates`](#0x3_staking_pool_exchange_rates)
-  [Function `iota_amount`](#0x3_staking_pool_iota_amount)
-  [Function `pool_token_amount`](#0x3_staking_pool_pool_token_amount)
-  [Function `is_preactive_at_epoch`](#0x3_staking_pool_is_preactive_at_epoch)
-  [Function `get_iota_amount`](#0x3_staking_pool_get_iota_amount)
-  [Function `get_token_amount`](#0x3_staking_pool_get_token_amount)
-  [Function `initial_exchange_rate`](#0x3_staking_pool_initial_exchange_rate)
-  [Function `check_balance_invariants`](#0x3_staking_pool_check_balance_invariants)


<pre><code><b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/u64.md#0x1_u64">0x1::u64</a>;
<b>use</b> <a href="../iota-framework/bag.md#0x2_bag">0x2::bag</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/iota.md#0x2_iota">0x2::iota</a>;
<b>use</b> <a href="../iota-framework/table.md#0x2_table">0x2::table</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x3_staking_pool_StakingPool"></a>

## Resource `StakingPool`

A staking pool embedded in each validator struct in the system state object.


<pre><code><b>struct</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a> <b>has</b> store, key
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
<code>activation_epoch: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>&gt;</code>
</dt>
<dd>
 The epoch at which this pool became active.
 The value is <code>None</code> if the pool is pre-active and <code>Some(&lt;epoch_number&gt;)</code> if active or inactive.
</dd>
<dt>
<code>deactivation_epoch: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>&gt;</code>
</dt>
<dd>
 The epoch at which this staking pool ceased to be active. <code>None</code> = {pre-active, active},
 <code>Some(&lt;epoch_number&gt;)</code> if in-active, and it was de-activated at epoch <code>&lt;epoch_number&gt;</code>.
</dd>
<dt>
<code>iota_balance: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 The total number of IOTA tokens in this pool, including the IOTA in the rewards_pool, as well as in all the principal
 in the <code><a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a></code> object, updated at epoch boundaries.
</dd>
<dt>
<code>rewards_pool: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;</code>
</dt>
<dd>
 The epoch stake rewards will be added here at the end of each epoch.
</dd>
<dt>
<code>pool_token_balance: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 Total number of pool tokens issued by the pool.
</dd>
<dt>
<code>exchange_rates: <a href="../iota-framework/table.md#0x2_table_Table">table::Table</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>&gt;</code>
</dt>
<dd>
 Exchange rate history of previous epochs. Key is the epoch number.
 The entries start from the <code>activation_epoch</code> of this pool and contains exchange rates at the beginning of each epoch,
 i.e., right after the rewards for the previous epoch have been deposited into the pool.
</dd>
<dt>
<code>pending_stake: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 Pending stake amount for this epoch, emptied at epoch boundaries.
</dd>
<dt>
<code>pending_total_iota_withdraw: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 Pending stake withdrawn during the current epoch, emptied at epoch boundaries.
 This includes both the principal and rewards IOTA withdrawn.
</dd>
<dt>
<code>pending_pool_token_withdraw: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 Pending pool token withdrawn during the current epoch, emptied at epoch boundaries.
</dd>
<dt>
<code>extra_fields: <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a></code>
</dt>
<dd>
 Any extra fields that's not defined statically.
</dd>
</dl>


</details>

<a name="0x3_staking_pool_PoolTokenExchangeRate"></a>

## Struct `PoolTokenExchangeRate`

Struct representing the exchange rate of the stake pool token to IOTA.


<pre><code><b>struct</b> <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a> <b>has</b> <b>copy</b>, drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>iota_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>

</dd>
<dt>
<code>pool_token_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x3_staking_pool_StakedIota"></a>

## Resource `StakedIota`

A self-custodial object holding the staked IOTA tokens.


<pre><code><b>struct</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> <b>has</b> store, key
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
<code>pool_id: <a href="../iota-framework/object.md#0x2_object_ID">object::ID</a></code>
</dt>
<dd>
 ID of the staking pool we are staking with.
</dd>
<dt>
<code>stake_activation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 The epoch at which the stake becomes active.
</dd>
<dt>
<code>principal: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;</code>
</dt>
<dd>
 The staked IOTA tokens.
</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x3_staking_pool_EActivationOfInactivePool"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EActivationOfInactivePool">EActivationOfInactivePool</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 16;
</code></pre>



<a name="0x3_staking_pool_EDeactivationOfInactivePool"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EDeactivationOfInactivePool">EDeactivationOfInactivePool</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 11;
</code></pre>



<a name="0x3_staking_pool_EDelegationOfZeroIota"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EDelegationOfZeroIota">EDelegationOfZeroIota</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 17;
</code></pre>



<a name="0x3_staking_pool_EDelegationToInactivePool"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EDelegationToInactivePool">EDelegationToInactivePool</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 10;
</code></pre>



<a name="0x3_staking_pool_EDestroyNonzeroBalance"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EDestroyNonzeroBalance">EDestroyNonzeroBalance</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 5;
</code></pre>



<a name="0x3_staking_pool_EIncompatibleStakedIota"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EIncompatibleStakedIota">EIncompatibleStakedIota</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 12;
</code></pre>



<a name="0x3_staking_pool_EInsufficientPoolTokenBalance"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EInsufficientPoolTokenBalance">EInsufficientPoolTokenBalance</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x3_staking_pool_EInsufficientRewardsPoolBalance"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EInsufficientRewardsPoolBalance">EInsufficientRewardsPoolBalance</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 4;
</code></pre>



<a name="0x3_staking_pool_EInsufficientIotaTokenBalance"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EInsufficientIotaTokenBalance">EInsufficientIotaTokenBalance</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 3;
</code></pre>



<a name="0x3_staking_pool_EPendingDelegationDoesNotExist"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EPendingDelegationDoesNotExist">EPendingDelegationDoesNotExist</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 8;
</code></pre>



<a name="0x3_staking_pool_EPoolAlreadyActive"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EPoolAlreadyActive">EPoolAlreadyActive</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 14;
</code></pre>



<a name="0x3_staking_pool_EPoolNotPreactive"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EPoolNotPreactive">EPoolNotPreactive</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 15;
</code></pre>



<a name="0x3_staking_pool_EStakedIotaBelowThreshold"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EStakedIotaBelowThreshold">EStakedIotaBelowThreshold</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 18;
</code></pre>



<a name="0x3_staking_pool_ETokenBalancesDoNotMatchExchangeRate"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_ETokenBalancesDoNotMatchExchangeRate">ETokenBalancesDoNotMatchExchangeRate</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 9;
</code></pre>



<a name="0x3_staking_pool_ETokenTimeLockIsSome"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_ETokenTimeLockIsSome">ETokenTimeLockIsSome</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 6;
</code></pre>



<a name="0x3_staking_pool_EWithdrawAmountCannotBeZero"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EWithdrawAmountCannotBeZero">EWithdrawAmountCannotBeZero</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 2;
</code></pre>



<a name="0x3_staking_pool_EWithdrawalInSameEpoch"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EWithdrawalInSameEpoch">EWithdrawalInSameEpoch</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 13;
</code></pre>



<a name="0x3_staking_pool_EWrongDelegation"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EWrongDelegation">EWrongDelegation</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 7;
</code></pre>



<a name="0x3_staking_pool_EWrongPool"></a>



<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_EWrongPool">EWrongPool</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1;
</code></pre>



<a name="0x3_staking_pool_MIN_STAKING_THRESHOLD"></a>

StakedIota objects cannot be split to below this amount.


<pre><code><b>const</b> <a href="staking_pool.md#0x3_staking_pool_MIN_STAKING_THRESHOLD">MIN_STAKING_THRESHOLD</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1000000000;
</code></pre>



<a name="0x3_staking_pool_new"></a>

## Function `new`

Create a new, empty staking pool.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_new">new</a>(ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_new">new</a>(ctx: &<b>mut</b> TxContext) : <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a> {
    <b>let</b> exchange_rates = <a href="../iota-framework/table.md#0x2_table_new">table::new</a>(ctx);
    <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        activation_epoch: <a href="../move-stdlib/option.md#0x1_option_none">option::none</a>(),
        deactivation_epoch: <a href="../move-stdlib/option.md#0x1_option_none">option::none</a>(),
        iota_balance: 0,
        rewards_pool: <a href="../iota-framework/balance.md#0x2_balance_zero">balance::zero</a>(),
        pool_token_balance: 0,
        exchange_rates,
        pending_stake: 0,
        pending_total_iota_withdraw: 0,
        pending_pool_token_withdraw: 0,
        extra_fields: <a href="../iota-framework/bag.md#0x2_bag_new">bag::new</a>(ctx),
    }
}
</code></pre>



</details>

<a name="0x3_staking_pool_request_add_stake"></a>

## Function `request_add_stake`

Request to stake to a staking pool. The stake starts counting at the beginning of the next epoch,


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_request_add_stake">request_add_stake</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, stake: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, stake_activation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_request_add_stake">request_add_stake</a>(
    pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>,
    stake: Balance&lt;IOTA&gt;,
    stake_activation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext
) : <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> {
    <b>let</b> iota_amount = stake.value();
    <b>assert</b>!(!<a href="staking_pool.md#0x3_staking_pool_is_inactive">is_inactive</a>(pool), <a href="staking_pool.md#0x3_staking_pool_EDelegationToInactivePool">EDelegationToInactivePool</a>);
    <b>assert</b>!(iota_amount &gt; 0, <a href="staking_pool.md#0x3_staking_pool_EDelegationOfZeroIota">EDelegationOfZeroIota</a>);
    <b>let</b> staked_iota = <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        pool_id: <a href="../iota-framework/object.md#0x2_object_id">object::id</a>(pool),
        stake_activation_epoch,
        principal: stake,
    };
    pool.pending_stake = pool.pending_stake + iota_amount;
    staked_iota
}
</code></pre>



</details>

<a name="0x3_staking_pool_request_withdraw_stake"></a>

## Function `request_withdraw_stake`

Request to withdraw the given stake plus rewards from a staking pool.
Both the principal and corresponding rewards in IOTA are withdrawn.
A proportional amount of pool token withdraw is recorded and processed at epoch change time.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_request_withdraw_stake">request_withdraw_stake</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_request_withdraw_stake">request_withdraw_stake</a>(
    pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>,
    staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>,
    ctx: &TxContext
) : Balance&lt;IOTA&gt; {
    // stake is inactive
    <b>if</b> (staked_iota.stake_activation_epoch &gt; ctx.epoch()) {
        <b>let</b> principal = <a href="staking_pool.md#0x3_staking_pool_unwrap_staked_iota">unwrap_staked_iota</a>(staked_iota);
        pool.pending_stake = pool.pending_stake - principal.value();

        <b>return</b> principal
    };

    <b>let</b> (pool_token_withdraw_amount, <b>mut</b> principal_withdraw) =
        <a href="staking_pool.md#0x3_staking_pool_withdraw_from_principal">withdraw_from_principal</a>(pool, staked_iota);
    <b>let</b> principal_withdraw_amount = principal_withdraw.value();

    <b>let</b> rewards_withdraw = <a href="staking_pool.md#0x3_staking_pool_withdraw_rewards">withdraw_rewards</a>(
        pool, principal_withdraw_amount, pool_token_withdraw_amount, ctx.epoch()
    );
    <b>let</b> total_iota_withdraw_amount = principal_withdraw_amount + rewards_withdraw.value();

    pool.pending_total_iota_withdraw = pool.pending_total_iota_withdraw + total_iota_withdraw_amount;
    pool.pending_pool_token_withdraw = pool.pending_pool_token_withdraw + pool_token_withdraw_amount;

    // If the pool is inactive, we immediately process the withdrawal.
    <b>if</b> (<a href="staking_pool.md#0x3_staking_pool_is_inactive">is_inactive</a>(pool)) <a href="staking_pool.md#0x3_staking_pool_process_pending_stake_withdraw">process_pending_stake_withdraw</a>(pool);

    // TODO: implement withdraw bonding period here.
    principal_withdraw.join(rewards_withdraw);
    principal_withdraw
}
</code></pre>



</details>

<a name="0x3_staking_pool_withdraw_from_principal"></a>

## Function `withdraw_from_principal`

Withdraw the principal IOTA stored in the StakedIota object, and calculate the corresponding amount of pool
tokens using exchange rate at staking epoch.
Returns values are amount of pool tokens withdrawn and withdrawn principal portion of IOTA.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_withdraw_from_principal">withdraw_from_principal</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>): (<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_withdraw_from_principal">withdraw_from_principal</a>(
    pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>,
    staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>,
) : (<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, Balance&lt;IOTA&gt;) {

    // Check that the stake information matches the pool.
    <b>assert</b>!(staked_iota.pool_id == <a href="../iota-framework/object.md#0x2_object_id">object::id</a>(pool), <a href="staking_pool.md#0x3_staking_pool_EWrongPool">EWrongPool</a>);

    <b>let</b> exchange_rate_at_staking_epoch = <a href="staking_pool.md#0x3_staking_pool_pool_token_exchange_rate_at_epoch">pool_token_exchange_rate_at_epoch</a>(pool, staked_iota.stake_activation_epoch);
    <b>let</b> principal_withdraw = <a href="staking_pool.md#0x3_staking_pool_unwrap_staked_iota">unwrap_staked_iota</a>(staked_iota);
    <b>let</b> pool_token_withdraw_amount = <a href="staking_pool.md#0x3_staking_pool_get_token_amount">get_token_amount</a>(
		&exchange_rate_at_staking_epoch,
		principal_withdraw.value()
	);

    (
        pool_token_withdraw_amount,
        principal_withdraw,
    )
}
</code></pre>



</details>

<a name="0x3_staking_pool_unwrap_staked_iota"></a>

## Function `unwrap_staked_iota`



<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_unwrap_staked_iota">unwrap_staked_iota</a>(staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_unwrap_staked_iota">unwrap_staked_iota</a>(staked_iota: <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>): Balance&lt;IOTA&gt; {
    <b>let</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> {
        id,
        pool_id: _,
        stake_activation_epoch: _,
        principal,
    } = staked_iota;
    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);
    principal
}
</code></pre>



</details>

<a name="0x3_staking_pool_deposit_rewards"></a>

## Function `deposit_rewards`

Called at epoch advancement times to add rewards (in IOTA) to the staking pool.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_deposit_rewards">deposit_rewards</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, rewards: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_deposit_rewards">deposit_rewards</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, rewards: Balance&lt;IOTA&gt;) {
    pool.iota_balance = pool.iota_balance + rewards.value();
    pool.rewards_pool.join(rewards);
}
</code></pre>



</details>

<a name="0x3_staking_pool_process_pending_stakes_and_withdraws"></a>

## Function `process_pending_stakes_and_withdraws`



<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_process_pending_stakes_and_withdraws">process_pending_stakes_and_withdraws</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_process_pending_stakes_and_withdraws">process_pending_stakes_and_withdraws</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, ctx: &TxContext) {
    <b>let</b> new_epoch = ctx.epoch() + 1;
    <a href="staking_pool.md#0x3_staking_pool_process_pending_stake_withdraw">process_pending_stake_withdraw</a>(pool);
    <a href="staking_pool.md#0x3_staking_pool_process_pending_stake">process_pending_stake</a>(pool);
    pool.exchange_rates.add(
        new_epoch,
        <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a> { iota_amount: pool.iota_balance, pool_token_amount: pool.pool_token_balance },
    );
    <a href="staking_pool.md#0x3_staking_pool_check_balance_invariants">check_balance_invariants</a>(pool, new_epoch);
}
</code></pre>



</details>

<a name="0x3_staking_pool_process_pending_stake_withdraw"></a>

## Function `process_pending_stake_withdraw`

Called at epoch boundaries to process pending stake withdraws requested during the epoch.
Also called immediately upon withdrawal if the pool is inactive.


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_process_pending_stake_withdraw">process_pending_stake_withdraw</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_process_pending_stake_withdraw">process_pending_stake_withdraw</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>) {
    pool.iota_balance = pool.iota_balance - pool.pending_total_iota_withdraw;
    pool.pool_token_balance = pool.pool_token_balance - pool.pending_pool_token_withdraw;
    pool.pending_total_iota_withdraw = 0;
    pool.pending_pool_token_withdraw = 0;
}
</code></pre>



</details>

<a name="0x3_staking_pool_process_pending_stake"></a>

## Function `process_pending_stake`

Called at epoch boundaries to process the pending stake.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_process_pending_stake">process_pending_stake</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_process_pending_stake">process_pending_stake</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>) {
    // Use the most up <b>to</b> date exchange rate <b>with</b> the rewards deposited and withdraws effectuated.
    <b>let</b> latest_exchange_rate =
        <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a> { iota_amount: pool.iota_balance, pool_token_amount: pool.pool_token_balance };
    pool.iota_balance = pool.iota_balance + pool.pending_stake;
    pool.pool_token_balance = <a href="staking_pool.md#0x3_staking_pool_get_token_amount">get_token_amount</a>(&latest_exchange_rate, pool.iota_balance);
    pool.pending_stake = 0;
}
</code></pre>



</details>

<a name="0x3_staking_pool_withdraw_rewards"></a>

## Function `withdraw_rewards`

This function does the following:
1. Calculates the total amount of IOTA (including principal and rewards) that the provided pool tokens represent
at the current exchange rate.
2. Using the above number and the given <code>principal_withdraw_amount</code>, calculates the rewards portion of the
stake we should withdraw.
3. Withdraws the rewards portion from the rewards pool at the current exchange rate. We only withdraw the rewards
portion because the principal portion was already taken out of the staker's self custodied StakedIota.


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_withdraw_rewards">withdraw_rewards</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, principal_withdraw_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, pool_token_withdraw_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_withdraw_rewards">withdraw_rewards</a>(
    pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>,
    principal_withdraw_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    pool_token_withdraw_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
) : Balance&lt;IOTA&gt; {
    <b>let</b> exchange_rate = <a href="staking_pool.md#0x3_staking_pool_pool_token_exchange_rate_at_epoch">pool_token_exchange_rate_at_epoch</a>(pool, epoch);
    <b>let</b> total_iota_withdraw_amount = <a href="staking_pool.md#0x3_staking_pool_get_iota_amount">get_iota_amount</a>(&exchange_rate, pool_token_withdraw_amount);
    <b>let</b> <b>mut</b> reward_withdraw_amount =
        <b>if</b> (total_iota_withdraw_amount &gt;= principal_withdraw_amount)
            total_iota_withdraw_amount - principal_withdraw_amount
        <b>else</b> 0;
    // This may happen when we are withdrawing everything from the pool and
    // the rewards pool <a href="../iota-framework/balance.md#0x2_balance">balance</a> may be less than reward_withdraw_amount.
    // TODO: FIGURE OUT EXACTLY WHY THIS CAN HAPPEN.
    reward_withdraw_amount = reward_withdraw_amount.<b>min</b>(pool.rewards_pool.value());
    pool.rewards_pool.<a href="staking_pool.md#0x3_staking_pool_split">split</a>(reward_withdraw_amount)
}
</code></pre>



</details>

<a name="0x3_staking_pool_activate_staking_pool"></a>

## Function `activate_staking_pool`

Called by <code><a href="validator.md#0x3_validator">validator</a></code> module to activate a staking pool.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_activate_staking_pool">activate_staking_pool</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, activation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_activate_staking_pool">activate_staking_pool</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, activation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>) {
    // Add the initial exchange rate <b>to</b> the <a href="../iota-framework/table.md#0x2_table">table</a>.
    pool.exchange_rates.add(
        activation_epoch,
        <a href="staking_pool.md#0x3_staking_pool_initial_exchange_rate">initial_exchange_rate</a>()
    );
    // Check that the pool is preactive and not inactive.
    <b>assert</b>!(<a href="staking_pool.md#0x3_staking_pool_is_preactive">is_preactive</a>(pool), <a href="staking_pool.md#0x3_staking_pool_EPoolAlreadyActive">EPoolAlreadyActive</a>);
    <b>assert</b>!(!<a href="staking_pool.md#0x3_staking_pool_is_inactive">is_inactive</a>(pool), <a href="staking_pool.md#0x3_staking_pool_EActivationOfInactivePool">EActivationOfInactivePool</a>);
    // Fill in the active epoch.
    pool.activation_epoch.fill(activation_epoch);
}
</code></pre>



</details>

<a name="0x3_staking_pool_deactivate_staking_pool"></a>

## Function `deactivate_staking_pool`

Deactivate a staking pool by setting the <code>deactivation_epoch</code>. After
this pool deactivation, the pool stops earning rewards. Only stake
withdraws can be made to the pool.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_deactivate_staking_pool">deactivate_staking_pool</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, deactivation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_deactivate_staking_pool">deactivate_staking_pool</a>(pool: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, deactivation_epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>) {
    // We can't deactivate an already deactivated pool.
    <b>assert</b>!(!<a href="staking_pool.md#0x3_staking_pool_is_inactive">is_inactive</a>(pool), <a href="staking_pool.md#0x3_staking_pool_EDeactivationOfInactivePool">EDeactivationOfInactivePool</a>);
    pool.deactivation_epoch = <a href="../move-stdlib/option.md#0x1_option_some">option::some</a>(deactivation_epoch);
}
</code></pre>



</details>

<a name="0x3_staking_pool_iota_balance"></a>

## Function `iota_balance`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_iota_balance">iota_balance</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_iota_balance">iota_balance</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> { pool.iota_balance }
</code></pre>



</details>

<a name="0x3_staking_pool_pool_id"></a>

## Function `pool_id`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pool_id">pool_id</a>(staked_iota: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>): <a href="../iota-framework/object.md#0x2_object_ID">object::ID</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pool_id">pool_id</a>(staked_iota: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>): ID { staked_iota.pool_id }
</code></pre>



</details>

<a name="0x3_staking_pool_staked_iota_amount"></a>

## Function `staked_iota_amount`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_staked_iota_amount">staked_iota_amount</a>(staked_iota: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_staked_iota_amount">staked_iota_amount</a>(staked_iota: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> { staked_iota.principal.value() }
</code></pre>



</details>

<a name="0x3_staking_pool_stake_activation_epoch"></a>

## Function `stake_activation_epoch`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_stake_activation_epoch">stake_activation_epoch</a>(staked_iota: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_stake_activation_epoch">stake_activation_epoch</a>(staked_iota: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    staked_iota.stake_activation_epoch
}
</code></pre>



</details>

<a name="0x3_staking_pool_is_preactive"></a>

## Function `is_preactive`

Returns true if the input staking pool is preactive.


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_preactive">is_preactive</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_preactive">is_preactive</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>): bool{
    pool.activation_epoch.is_none()
}
</code></pre>



</details>

<a name="0x3_staking_pool_is_inactive"></a>

## Function `is_inactive`

Returns true if the input staking pool is inactive.


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_inactive">is_inactive</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_inactive">is_inactive</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>): bool {
    pool.deactivation_epoch.is_some()
}
</code></pre>



</details>

<a name="0x3_staking_pool_split"></a>

## Function `split`

Split StakedIota <code>self</code> to two parts, one with principal <code>split_amount</code>,
and the remaining principal is left in <code>self</code>.
All the other parameters of the StakedIota like <code>stake_activation_epoch</code> or <code>pool_id</code> remain the same.


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_split">split</a>(self: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_split">split</a>(self: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext): <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> {
    <b>let</b> original_amount = self.principal.value();
    <b>assert</b>!(split_amount &lt;= original_amount, <a href="staking_pool.md#0x3_staking_pool_EInsufficientIotaTokenBalance">EInsufficientIotaTokenBalance</a>);
    <b>let</b> remaining_amount = original_amount - split_amount;
    // Both resulting parts should have at least <a href="staking_pool.md#0x3_staking_pool_MIN_STAKING_THRESHOLD">MIN_STAKING_THRESHOLD</a>.
    <b>assert</b>!(remaining_amount &gt;= <a href="staking_pool.md#0x3_staking_pool_MIN_STAKING_THRESHOLD">MIN_STAKING_THRESHOLD</a>, <a href="staking_pool.md#0x3_staking_pool_EStakedIotaBelowThreshold">EStakedIotaBelowThreshold</a>);
    <b>assert</b>!(split_amount &gt;= <a href="staking_pool.md#0x3_staking_pool_MIN_STAKING_THRESHOLD">MIN_STAKING_THRESHOLD</a>, <a href="staking_pool.md#0x3_staking_pool_EStakedIotaBelowThreshold">EStakedIotaBelowThreshold</a>);
    <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        pool_id: self.pool_id,
        stake_activation_epoch: self.stake_activation_epoch,
        principal: self.principal.<a href="staking_pool.md#0x3_staking_pool_split">split</a>(split_amount),
    }
}
</code></pre>



</details>

<a name="0x3_staking_pool_split_staked_iota"></a>

## Function `split_staked_iota`

Split the given StakedIota to the two parts, one with principal <code>split_amount</code>,
transfer the newly split part to the sender address.


<pre><code><b>public</b> entry <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_split_staked_iota">split_staked_iota</a>(stake: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_split_staked_iota">split_staked_iota</a>(stake: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>, split_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(<a href="staking_pool.md#0x3_staking_pool_split">split</a>(stake, split_amount, ctx), ctx.sender());
}
</code></pre>



</details>

<a name="0x3_staking_pool_join_staked_iota"></a>

## Function `join_staked_iota`

Consume the staked iota <code>other</code> and add its value to <code>self</code>.
Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)


<pre><code><b>public</b> entry <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_join_staked_iota">join_staked_iota</a>(self: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, other: <a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_join_staked_iota">join_staked_iota</a>(self: &<b>mut</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>, other: <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>) {
    <b>assert</b>!(<a href="staking_pool.md#0x3_staking_pool_is_equal_staking_metadata">is_equal_staking_metadata</a>(self, &other), <a href="staking_pool.md#0x3_staking_pool_EIncompatibleStakedIota">EIncompatibleStakedIota</a>);
    <b>let</b> <a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a> {
        id,
        pool_id: _,
        stake_activation_epoch: _,
        principal,
    } = other;

    id.delete();
    self.principal.join(principal);
}
</code></pre>



</details>

<a name="0x3_staking_pool_is_equal_staking_metadata"></a>

## Function `is_equal_staking_metadata`

Returns true if all the staking parameters of the staked iota except the principal are identical


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, other: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>, other: &<a href="staking_pool.md#0x3_staking_pool_StakedIota">StakedIota</a>): bool {
    (self.pool_id == other.pool_id) &&
    (self.stake_activation_epoch == other.stake_activation_epoch)
}
</code></pre>



</details>

<a name="0x3_staking_pool_pool_token_exchange_rate_at_epoch"></a>

## Function `pool_token_exchange_rate_at_epoch`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pool_token_exchange_rate_at_epoch">pool_token_exchange_rate_at_epoch</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pool_token_exchange_rate_at_epoch">pool_token_exchange_rate_at_epoch</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a> {
    // If the pool is preactive then the exchange rate is always 1:1.
    <b>if</b> (<a href="staking_pool.md#0x3_staking_pool_is_preactive_at_epoch">is_preactive_at_epoch</a>(pool, epoch)) {
        <b>return</b> <a href="staking_pool.md#0x3_staking_pool_initial_exchange_rate">initial_exchange_rate</a>()
    };
    <b>let</b> clamped_epoch = pool.deactivation_epoch.get_with_default(epoch);
    <b>let</b> <b>mut</b> epoch = clamped_epoch.<b>min</b>(epoch);
    <b>let</b> activation_epoch = *pool.activation_epoch.borrow();

    // Find the latest epoch that's earlier than the given epoch <b>with</b> an entry in the <a href="../iota-framework/table.md#0x2_table">table</a>
    <b>while</b> (epoch &gt;= activation_epoch) {
        <b>if</b> (pool.exchange_rates.contains(epoch)) {
            <b>return</b> pool.exchange_rates[epoch]
        };
        epoch = epoch - 1;
    };
    // This line really should be unreachable. Do we want an <b>assert</b> <b>false</b> here?
    <a href="staking_pool.md#0x3_staking_pool_initial_exchange_rate">initial_exchange_rate</a>()
}
</code></pre>



</details>

<a name="0x3_staking_pool_pending_stake_amount"></a>

## Function `pending_stake_amount`

Returns the total value of the pending staking requests for this staking pool.


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pending_stake_amount">pending_stake_amount</a>(<a href="staking_pool.md#0x3_staking_pool">staking_pool</a>: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pending_stake_amount">pending_stake_amount</a>(<a href="staking_pool.md#0x3_staking_pool">staking_pool</a>: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    <a href="staking_pool.md#0x3_staking_pool">staking_pool</a>.pending_stake
}
</code></pre>



</details>

<a name="0x3_staking_pool_pending_stake_withdraw_amount"></a>

## Function `pending_stake_withdraw_amount`

Returns the total withdrawal from the staking pool this epoch.


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pending_stake_withdraw_amount">pending_stake_withdraw_amount</a>(<a href="staking_pool.md#0x3_staking_pool">staking_pool</a>: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pending_stake_withdraw_amount">pending_stake_withdraw_amount</a>(<a href="staking_pool.md#0x3_staking_pool">staking_pool</a>: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    <a href="staking_pool.md#0x3_staking_pool">staking_pool</a>.pending_total_iota_withdraw
}
</code></pre>



</details>

<a name="0x3_staking_pool_exchange_rates"></a>

## Function `exchange_rates`



<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_exchange_rates">exchange_rates</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>): &<a href="../iota-framework/table.md#0x2_table_Table">table::Table</a>&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_exchange_rates">exchange_rates</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>): &Table&lt;<a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a>&gt; {
    &pool.exchange_rates
}
</code></pre>



</details>

<a name="0x3_staking_pool_iota_amount"></a>

## Function `iota_amount`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_iota_amount">iota_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_iota_amount">iota_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    exchange_rate.iota_amount
}
</code></pre>



</details>

<a name="0x3_staking_pool_pool_token_amount"></a>

## Function `pool_token_amount`



<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pool_token_amount">pool_token_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="staking_pool.md#0x3_staking_pool_pool_token_amount">pool_token_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    exchange_rate.pool_token_amount
}
</code></pre>



</details>

<a name="0x3_staking_pool_is_preactive_at_epoch"></a>

## Function `is_preactive_at_epoch`

Returns true if the provided staking pool is preactive at the provided epoch.


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_preactive_at_epoch">is_preactive_at_epoch</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_is_preactive_at_epoch">is_preactive_at_epoch</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): bool{
    // Either the pool is currently preactive or the pool's starting epoch is later than the provided epoch.
    <a href="staking_pool.md#0x3_staking_pool_is_preactive">is_preactive</a>(pool) || (*pool.activation_epoch.borrow() &gt; epoch)
}
</code></pre>



</details>

<a name="0x3_staking_pool_get_iota_amount"></a>

## Function `get_iota_amount`



<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_get_iota_amount">get_iota_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>, token_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_get_iota_amount">get_iota_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a>, token_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    // When either amount is 0, that means we have no stakes <b>with</b> this pool.
    // The other amount might be non-zero when there's dust left in the pool.
    <b>if</b> (exchange_rate.iota_amount == 0 || exchange_rate.pool_token_amount == 0) {
        <b>return</b> token_amount
    };
    <b>let</b> res = exchange_rate.iota_amount <b>as</b> u128
            * (token_amount <b>as</b> u128)
            / (exchange_rate.pool_token_amount <b>as</b> u128);
    res <b>as</b> <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
}
</code></pre>



</details>

<a name="0x3_staking_pool_get_token_amount"></a>

## Function `get_token_amount`



<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_get_token_amount">get_token_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>, iota_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_get_token_amount">get_token_amount</a>(exchange_rate: &<a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a>, iota_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    // When either amount is 0, that means we have no stakes <b>with</b> this pool.
    // The other amount might be non-zero when there's dust left in the pool.
    <b>if</b> (exchange_rate.iota_amount == 0 || exchange_rate.pool_token_amount == 0) {
        <b>return</b> iota_amount
    };
    <b>let</b> res = exchange_rate.pool_token_amount <b>as</b> u128
            * (iota_amount <b>as</b> u128)
            / (exchange_rate.iota_amount <b>as</b> u128);
    res <b>as</b> <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
}
</code></pre>



</details>

<a name="0x3_staking_pool_initial_exchange_rate"></a>

## Function `initial_exchange_rate`



<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_initial_exchange_rate">initial_exchange_rate</a>(): <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">staking_pool::PoolTokenExchangeRate</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_initial_exchange_rate">initial_exchange_rate</a>(): <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a> {
    <a href="staking_pool.md#0x3_staking_pool_PoolTokenExchangeRate">PoolTokenExchangeRate</a> { iota_amount: 0, pool_token_amount: 0 }
}
</code></pre>



</details>

<a name="0x3_staking_pool_check_balance_invariants"></a>

## Function `check_balance_invariants`



<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_check_balance_invariants">check_balance_invariants</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">staking_pool::StakingPool</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="staking_pool.md#0x3_staking_pool_check_balance_invariants">check_balance_invariants</a>(pool: &<a href="staking_pool.md#0x3_staking_pool_StakingPool">StakingPool</a>, epoch: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>) {
    <b>let</b> exchange_rate = <a href="staking_pool.md#0x3_staking_pool_pool_token_exchange_rate_at_epoch">pool_token_exchange_rate_at_epoch</a>(pool, epoch);
    // check that the pool token <a href="../iota-framework/balance.md#0x2_balance">balance</a> and <a href="../iota-framework/iota.md#0x2_iota">iota</a> <a href="../iota-framework/balance.md#0x2_balance">balance</a> ratio matches the exchange rate stored.
    <b>let</b> expected = <a href="staking_pool.md#0x3_staking_pool_get_token_amount">get_token_amount</a>(&exchange_rate, pool.iota_balance);
    <b>let</b> actual = pool.pool_token_balance;
    <b>assert</b>!(expected == actual, <a href="staking_pool.md#0x3_staking_pool_ETokenBalancesDoNotMatchExchangeRate">ETokenBalancesDoNotMatchExchangeRate</a>)
}
</code></pre>



</details>
