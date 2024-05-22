---
title: Module `0x10cf::timelocked_staked_sui`
---



-  [Resource `TimelockedStakedSui`](#0x10cf_timelocked_staked_sui_TimelockedStakedSui)
-  [Constants](#@Constants_0)
-  [Function `create`](#0x10cf_timelocked_staked_sui_create)
-  [Function `pool_id`](#0x10cf_timelocked_staked_sui_pool_id)
-  [Function `staked_sui_amount`](#0x10cf_timelocked_staked_sui_staked_sui_amount)
-  [Function `stake_activation_epoch`](#0x10cf_timelocked_staked_sui_stake_activation_epoch)
-  [Function `expiration_timestamp_ms`](#0x10cf_timelocked_staked_sui_expiration_timestamp_ms)
-  [Function `split`](#0x10cf_timelocked_staked_sui_split)
-  [Function `split_staked_sui`](#0x10cf_timelocked_staked_sui_split_staked_sui)
-  [Function `join_staked_sui`](#0x10cf_timelocked_staked_sui_join_staked_sui)
-  [Function `is_equal_staking_metadata`](#0x10cf_timelocked_staked_sui_is_equal_staking_metadata)
-  [Function `unpack`](#0x10cf_timelocked_staked_sui_unpack)
-  [Function `transfer`](#0x10cf_timelocked_staked_sui_transfer)


<pre><code><b>use</b> <a href="../sui-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../sui-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../sui-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../sui-system/staking_pool.md#0x3_staking_pool">0x3::staking_pool</a>;
</code></pre>



<a name="0x10cf_timelocked_staked_sui_TimelockedStakedSui"></a>

## Resource `TimelockedStakedSui`

A self-custodial object holding the timelocked staked SUI tokens.


<pre><code><b>struct</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../sui-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>

</dd>
<dt>
<code>staked_sui: <a href="../sui-system/staking_pool.md#0x3_staking_pool_StakedSui">staking_pool::StakedSui</a></code>
</dt>
<dd>
 A self-custodial object holding the staked SUI tokens.
</dd>
<dt>
<code>expiration_timestamp_ms: u64</code>
</dt>
<dd>
 This is the epoch time stamp of when the lock expires.
</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x10cf_timelocked_staked_sui_EIncompatibleTimelockedStakedSui"></a>



<pre><code><b>const</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_EIncompatibleTimelockedStakedSui">EIncompatibleTimelockedStakedSui</a>: u64 = 0;
</code></pre>



<a name="0x10cf_timelocked_staked_sui_create"></a>

## Function `create`

Create a new instance of <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_create">create</a>(staked_sui: <a href="../sui-system/staking_pool.md#0x3_staking_pool_StakedSui">staking_pool::StakedSui</a>, expiration_timestamp_ms: u64, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_create">create</a>(
    staked_sui: StakedSui,
    expiration_timestamp_ms: u64,
    ctx: &<b>mut</b> TxContext
): <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> {
    <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> {
        id: <a href="../sui-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_sui,
        expiration_timestamp_ms
    }
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_pool_id"></a>

## Function `pool_id`

Function to get the pool id of a <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_pool_id">pool_id</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>): <a href="../sui-framework/object.md#0x2_object_ID">object::ID</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_pool_id">pool_id</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>): ID { self.staked_sui.<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_pool_id">pool_id</a>() }
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_staked_sui_amount"></a>

## Function `staked_sui_amount`

Function to get the staked sui amount of a <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_staked_sui_amount">staked_sui_amount</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_staked_sui_amount">staked_sui_amount</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>): u64 { self.staked_sui.<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_staked_sui_amount">staked_sui_amount</a>() }
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_stake_activation_epoch"></a>

## Function `stake_activation_epoch`

Function to get the stake activation epoch of a <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_stake_activation_epoch">stake_activation_epoch</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_stake_activation_epoch">stake_activation_epoch</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>): u64 {
    self.staked_sui.<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_stake_activation_epoch">stake_activation_epoch</a>()
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_expiration_timestamp_ms"></a>

## Function `expiration_timestamp_ms`

Function to get the expiration timestamp of a <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_expiration_timestamp_ms">expiration_timestamp_ms</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_expiration_timestamp_ms">expiration_timestamp_ms</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>): u64 {
    self.expiration_timestamp_ms
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_split"></a>

## Function `split`

Split <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code> into two parts, one with principal <code>split_amount</code>,
and the remaining principal is left in <code>self</code>.
All the other parameters of the <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code> like <code>stake_activation_epoch</code> or <code>pool_id</code> remain the same.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_split">split</a>(self: &<b>mut</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, split_amount: u64, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_split">split</a>(self: &<b>mut</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>, split_amount: u64, ctx: &<b>mut</b> TxContext): <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> {
    <b>let</b> splitted_stake = self.staked_sui.<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_split">split</a>(split_amount, ctx);

    <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> {
        id: <a href="../sui-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_sui: splitted_stake,
        expiration_timestamp_ms: self.expiration_timestamp_ms,
    }
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_split_staked_sui"></a>

## Function `split_staked_sui`

Split the given <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code> to the two parts, one with principal <code>split_amount</code>,
transfer the newly split part to the sender address.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_split_staked_sui">split_staked_sui</a>(stake: &<b>mut</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, split_amount: u64, ctx: &<b>mut</b> <a href="../sui-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_split_staked_sui">split_staked_sui</a>(stake: &<b>mut</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>, split_amount: u64, ctx: &<b>mut</b> TxContext) {
    <a href="../sui-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_split">split</a>(stake, split_amount, ctx), ctx.sender());
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_join_staked_sui"></a>

## Function `join_staked_sui`

Consume the staked sui <code>other</code> and add its value to <code>self</code>.
Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_join_staked_sui">join_staked_sui</a>(self: &<b>mut</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, other: <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_join_staked_sui">join_staked_sui</a>(self: &<b>mut</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>, other: <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>) {
    <b>assert</b>!(self.<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_is_equal_staking_metadata">is_equal_staking_metadata</a>(&other), <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_EIncompatibleTimelockedStakedSui">EIncompatibleTimelockedStakedSui</a>);

    <b>let</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> {
        id,
        staked_sui,
        expiration_timestamp_ms: _,
    } = other;

    id.delete();

    self.staked_sui.join(staked_sui);
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_is_equal_staking_metadata"></a>

## Function `is_equal_staking_metadata`

Returns true if all the staking parameters of the staked sui except the principal are identical


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, other: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>, other: &<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>): bool {
    self.staked_sui.<a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_is_equal_staking_metadata">is_equal_staking_metadata</a>(&other.staked_sui) &&
    (self.expiration_timestamp_ms == other.expiration_timestamp_ms)
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_unpack"></a>

## Function `unpack`

An utility function to destroy a <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_unpack">unpack</a>(self: <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>): (<a href="../sui-system/staking_pool.md#0x3_staking_pool_StakedSui">staking_pool::StakedSui</a>, u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_unpack">unpack</a>(self: <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>): (StakedSui, u64) {
    <b>let</b> <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a> {
        id,
        staked_sui,
        expiration_timestamp_ms,
    } = self;

    <a href="../sui-framework/object.md#0x2_object_delete">object::delete</a>(id);

    (staked_sui, expiration_timestamp_ms)
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_sui_transfer"></a>

## Function `transfer`

An utility function to transfer a <code><a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="../sui-framework/transfer.md#0x2_transfer">transfer</a>(stake: <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">timelocked_staked_sui::TimelockedStakedSui</a>, recipient: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="../sui-framework/transfer.md#0x2_transfer">transfer</a>(stake: <a href="timelocked_staked_sui.md#0x10cf_timelocked_staked_sui_TimelockedStakedSui">TimelockedStakedSui</a>, recipient: <b>address</b>) {
    <a href="../sui-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(stake, recipient);
}
</code></pre>



</details>
