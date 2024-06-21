---
title: Module `0x10cf::timelocked_staked_iota`
---



-  [Resource `TimelockedStakedIota`](#0x10cf_timelocked_staked_iota_TimelockedStakedIota)
-  [Constants](#@Constants_0)
-  [Function `create`](#0x10cf_timelocked_staked_iota_create)
-  [Function `pool_id`](#0x10cf_timelocked_staked_iota_pool_id)
-  [Function `staked_iota_amount`](#0x10cf_timelocked_staked_iota_staked_iota_amount)
-  [Function `stake_activation_epoch`](#0x10cf_timelocked_staked_iota_stake_activation_epoch)
-  [Function `expiration_timestamp_ms`](#0x10cf_timelocked_staked_iota_expiration_timestamp_ms)
-  [Function `label`](#0x10cf_timelocked_staked_iota_label)
-  [Function `is_labeled_with`](#0x10cf_timelocked_staked_iota_is_labeled_with)
-  [Function `split`](#0x10cf_timelocked_staked_iota_split)
-  [Function `split_staked_iota`](#0x10cf_timelocked_staked_iota_split_staked_iota)
-  [Function `join_staked_iota`](#0x10cf_timelocked_staked_iota_join_staked_iota)
-  [Function `is_equal_staking_metadata`](#0x10cf_timelocked_staked_iota_is_equal_staking_metadata)
-  [Function `unpack`](#0x10cf_timelocked_staked_iota_unpack)
-  [Function `transfer`](#0x10cf_timelocked_staked_iota_transfer)
-  [Function `transfer_multiple`](#0x10cf_timelocked_staked_iota_transfer_multiple)


<pre><code><b>use</b> <a href="labeler.md#0x10cf_labeler">0x10cf::labeler</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../move-stdlib/vector.md#0x1_vector">0x1::vector</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-system/staking_pool.md#0x3_staking_pool">0x3::staking_pool</a>;
</code></pre>



<a name="0x10cf_timelocked_staked_iota_TimelockedStakedIota"></a>

## Resource `TimelockedStakedIota`

A self-custodial object holding the timelocked staked IOTA tokens.


<pre><code><b>struct</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> <b>has</b> key
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
<code>staked_iota: <a href="../iota-system/staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a></code>
</dt>
<dd>
 A self-custodial object holding the staked IOTA tokens.
</dd>
<dt>
<code>expiration_timestamp_ms: u64</code>
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


<a name="0x10cf_timelocked_staked_iota_EIncompatibleTimelockedStakedIota"></a>



<pre><code><b>const</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_EIncompatibleTimelockedStakedIota">EIncompatibleTimelockedStakedIota</a>: u64 = 0;
</code></pre>



<a name="0x10cf_timelocked_staked_iota_create"></a>

## Function `create`

Create a new instance of <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_create">create</a>(staked_iota: <a href="../iota-system/staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, expiration_timestamp_ms: u64, label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_create">create</a>(
    staked_iota: StakedIota,
    expiration_timestamp_ms: u64,
    label: Option&lt;String&gt;,
    ctx: &<b>mut</b> TxContext
): <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> {
    <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_iota,
        expiration_timestamp_ms,
        label,
    }
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_pool_id"></a>

## Function `pool_id`

Function to get the pool id of a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_pool_id">pool_id</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): <a href="../iota-framework/object.md#0x2_object_ID">object::ID</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_pool_id">pool_id</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): ID { self.staked_iota.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_pool_id">pool_id</a>() }
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_staked_iota_amount"></a>

## Function `staked_iota_amount`

Function to get the staked iota amount of a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_staked_iota_amount">staked_iota_amount</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_staked_iota_amount">staked_iota_amount</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): u64 { self.staked_iota.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_staked_iota_amount">staked_iota_amount</a>() }
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_stake_activation_epoch"></a>

## Function `stake_activation_epoch`

Function to get the stake activation epoch of a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_stake_activation_epoch">stake_activation_epoch</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_stake_activation_epoch">stake_activation_epoch</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): u64 {
    self.staked_iota.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_stake_activation_epoch">stake_activation_epoch</a>()
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_expiration_timestamp_ms"></a>

## Function `expiration_timestamp_ms`

Function to get the expiration timestamp of a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_expiration_timestamp_ms">expiration_timestamp_ms</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_expiration_timestamp_ms">expiration_timestamp_ms</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): u64 {
    self.expiration_timestamp_ms
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_label"></a>

## Function `label`

Function to get the label of a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_label">label</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_label">label</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): Option&lt;String&gt; {
    self.label
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_is_labeled_with"></a>

## Function `is_labeled_with`

Check if a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code> is labeled with the type <code>L</code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_is_labeled_with">is_labeled_with</a>&lt;L&gt;(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_is_labeled_with">is_labeled_with</a>&lt;L&gt;(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): bool {
    <b>if</b> (self.label.is_some()) {
        self.label.borrow() == <a href="labeler.md#0x10cf_labeler_type_name">labeler::type_name</a>&lt;L&gt;()
    }
    <b>else</b> {
        <b>false</b>
    }
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_split"></a>

## Function `split`

Split <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code> into two parts, one with principal <code>split_amount</code>,
and the remaining principal is left in <code>self</code>.
All the other parameters of the <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code> like <code>stake_activation_epoch</code> or <code>pool_id</code> remain the same.


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_split">split</a>(self: &<b>mut</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, split_amount: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_split">split</a>(self: &<b>mut</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>, split_amount: u64, ctx: &<b>mut</b> TxContext): <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> {
    <b>let</b> splitted_stake = self.staked_iota.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_split">split</a>(split_amount, ctx);

    <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        staked_iota: splitted_stake,
        expiration_timestamp_ms: self.expiration_timestamp_ms,
        label: self.label,
    }
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_split_staked_iota"></a>

## Function `split_staked_iota`

Split the given <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code> to the two parts, one with principal <code>split_amount</code>,
transfer the newly split part to the sender address.


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_split_staked_iota">split_staked_iota</a>(stake: &<b>mut</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, split_amount: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_split_staked_iota">split_staked_iota</a>(stake: &<b>mut</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>, split_amount: u64, ctx: &<b>mut</b> TxContext) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_split">split</a>(stake, split_amount, ctx), ctx.sender());
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_join_staked_iota"></a>

## Function `join_staked_iota`

Consume the staked iota <code>other</code> and add its value to <code>self</code>.
Aborts if some of the staking parameters are incompatible (pool id, stake activation epoch, etc.)


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_join_staked_iota">join_staked_iota</a>(self: &<b>mut</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, other: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_join_staked_iota">join_staked_iota</a>(self: &<b>mut</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>, other: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>) {
    <b>assert</b>!(self.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_is_equal_staking_metadata">is_equal_staking_metadata</a>(&other), <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_EIncompatibleTimelockedStakedIota">EIncompatibleTimelockedStakedIota</a>);

    <b>let</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> {
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

<a name="0x10cf_timelocked_staked_iota_is_equal_staking_metadata"></a>

## Function `is_equal_staking_metadata`

Returns true if all the staking parameters of the staked iota except the principal are identical


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, other: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_is_equal_staking_metadata">is_equal_staking_metadata</a>(self: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>, other: &<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): bool {
    self.staked_iota.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_is_equal_staking_metadata">is_equal_staking_metadata</a>(&other.staked_iota) &&
    (self.expiration_timestamp_ms == other.expiration_timestamp_ms) &&
    (self.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_label">label</a>() == other.<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_label">label</a>())
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_unpack"></a>

## Function `unpack`

A utility function to destroy a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_unpack">unpack</a>(self: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>): (<a href="../iota-system/staking_pool.md#0x3_staking_pool_StakedIota">staking_pool::StakedIota</a>, u64, <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_unpack">unpack</a>(self: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>): (StakedIota, u64, Option&lt;String&gt;) {
    <b>let</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a> {
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

<a name="0x10cf_timelocked_staked_iota_transfer"></a>

## Function `transfer`

A utility function to transfer a <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(stake: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>, recipient: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(stake: <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>, recipient: <b>address</b>) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(stake, recipient);
}
</code></pre>



</details>

<a name="0x10cf_timelocked_staked_iota_transfer_multiple"></a>

## Function `transfer_multiple`

A utility function to transfer multiple <code><a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_transfer_multiple">transfer_multiple</a>(stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">timelocked_staked_iota::TimelockedStakedIota</a>&gt;, recipient: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_transfer_multiple">transfer_multiple</a>(<b>mut</b> stakes: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelocked_staked_iota.md#0x10cf_timelocked_staked_iota_TimelockedStakedIota">TimelockedStakedIota</a>&gt;, recipient: <b>address</b>) {
    // Transfer all the time-locked stakes <b>to</b> the recipient.
    <b>while</b> (!stakes.is_empty()) {
       <b>let</b> stake = stakes.pop_back();
       <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(stake, recipient);
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(stakes);
}
</code></pre>



</details>
