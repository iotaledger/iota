---
title: Module `0x2::timelock`
---

A timelock implementation.


-  [Resource `TimeLock`](#0x2_timelock_TimeLock)
-  [Struct `SystemTimelockCap`](#0x2_timelock_SystemTimelockCap)
-  [Constants](#@Constants_0)
-  [Function `lock`](#0x2_timelock_lock)
-  [Function `lock_with_label`](#0x2_timelock_lock_with_label)
-  [Function `lock_and_transfer`](#0x2_timelock_lock_and_transfer)
-  [Function `lock_with_label_and_transfer`](#0x2_timelock_lock_with_label_and_transfer)
-  [Function `unlock`](#0x2_timelock_unlock)
-  [Function `join`](#0x2_timelock_join)
-  [Function `join_vec`](#0x2_timelock_join_vec)
-  [Function `split`](#0x2_timelock_split)
-  [Function `split_balance`](#0x2_timelock_split_balance)
-  [Function `transfer_to_sender`](#0x2_timelock_transfer_to_sender)
-  [Function `system_pack`](#0x2_timelock_system_pack)
-  [Function `system_unpack`](#0x2_timelock_system_unpack)
-  [Function `type_name`](#0x2_timelock_type_name)
-  [Function `expiration_timestamp_ms`](#0x2_timelock_expiration_timestamp_ms)
-  [Function `is_locked`](#0x2_timelock_is_locked)
-  [Function `remaining_time`](#0x2_timelock_remaining_time)
-  [Function `locked`](#0x2_timelock_locked)
-  [Function `label`](#0x2_timelock_label)
-  [Function `is_labeled_with`](#0x2_timelock_is_labeled_with)
-  [Function `pack`](#0x2_timelock_pack)
-  [Function `unpack`](#0x2_timelock_unpack)
-  [Function `transfer`](#0x2_timelock_transfer)
-  [Function `check_expiration_timestamp_ms`](#0x2_timelock_check_expiration_timestamp_ms)
-  [Function `new_system_timelock_cap`](#0x2_timelock_new_system_timelock_cap)


<pre><code><b>use</b> <a href="../move-stdlib/ascii.md#0x1_ascii">0x1::ascii</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../move-stdlib/type_name.md#0x1_type_name">0x1::type_name</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/labeler.md#0x2_labeler">0x2::labeler</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x2_timelock_TimeLock"></a>

## Resource `TimeLock`

<code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> struct that holds a locked object.


<pre><code><b>struct</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T: store&gt; <b>has</b> key
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
<code>locked: T</code>
</dt>
<dd>
 The locked object.
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

<a name="0x2_timelock_SystemTimelockCap"></a>

## Struct `SystemTimelockCap`

<code><a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a></code> allows to <code>pack</code> and <code>unpack</code> TimeLocks


<pre><code><b>struct</b> <a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a> <b>has</b> store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>dummy_field: bool</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x2_timelock_ENotSystemAddress"></a>

Sender is not @0x0 the system address.


<pre><code><b>const</b> <a href="../iota-framework/timelock.md#0x2_timelock_ENotSystemAddress">ENotSystemAddress</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1;
</code></pre>



<a name="0x2_timelock_EDifferentExpirationTime"></a>

For when trying to join two time-locked balances with different expiration time.


<pre><code><b>const</b> <a href="../iota-framework/timelock.md#0x2_timelock_EDifferentExpirationTime">EDifferentExpirationTime</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 4;
</code></pre>



<a name="0x2_timelock_EDifferentLabels"></a>

For when trying to join two time-locked balances with different labels.


<pre><code><b>const</b> <a href="../iota-framework/timelock.md#0x2_timelock_EDifferentLabels">EDifferentLabels</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 5;
</code></pre>



<a name="0x2_timelock_EExpireEpochIsPast"></a>

Expiration timestamp of the lock is in the past.


<pre><code><b>const</b> <a href="../iota-framework/timelock.md#0x2_timelock_EExpireEpochIsPast">EExpireEpochIsPast</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 2;
</code></pre>



<a name="0x2_timelock_ENotCalledAtGenesis"></a>

The <code>new</code> function was called at a non-genesis epoch.


<pre><code><b>const</b> <a href="../iota-framework/timelock.md#0x2_timelock_ENotCalledAtGenesis">ENotCalledAtGenesis</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x2_timelock_ENotExpiredYet"></a>

The lock has not expired yet.


<pre><code><b>const</b> <a href="../iota-framework/timelock.md#0x2_timelock_ENotExpiredYet">ENotExpiredYet</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 3;
</code></pre>



<a name="0x2_timelock_lock"></a>

## Function `lock`

Function to lock an object till a unix timestamp in milliseconds.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock">lock</a>&lt;T: store&gt;(locked: T, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock">lock</a>&lt;T: store&gt;(locked: T, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt; {
    // Check that `expiration_timestamp_ms` is valid.
    <a href="../iota-framework/timelock.md#0x2_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms, ctx);

    // Create a <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <a href="../iota-framework/timelock.md#0x2_timelock_pack">pack</a>(locked, expiration_timestamp_ms, <a href="../move-stdlib/option.md#0x1_option_none">option::none</a>(), ctx)
}
</code></pre>



</details>

<a name="0x2_timelock_lock_with_label"></a>

## Function `lock_with_label`

Function to lock a labeled object till a unix timestamp in milliseconds.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock_with_label">lock_with_label</a>&lt;T: store, L&gt;(_: &<a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;, locked: T, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock_with_label">lock_with_label</a>&lt;T: store, L&gt;(
    _: &LabelerCap&lt;L&gt;,
    locked: T,
    expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext
): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt; {
    // Check that `expiration_timestamp_ms` is valid.
    <a href="../iota-framework/timelock.md#0x2_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms, ctx);

    // Calculate a label value.
    <b>let</b> label = <a href="../move-stdlib/type_name.md#0x1_type_name">type_name</a>&lt;L&gt;();

    // Create a labeled <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <a href="../iota-framework/timelock.md#0x2_timelock_pack">pack</a>(locked, expiration_timestamp_ms, <a href="../move-stdlib/option.md#0x1_option_some">option::some</a>(label), ctx)
}
</code></pre>



</details>

<a name="0x2_timelock_lock_and_transfer"></a>

## Function `lock_and_transfer`

Function to lock an object <code>obj</code> until <code>expiration_timestamp_ms</code> and transfer it to address <code><b>to</b></code>.
Since <code>Timelock&lt;T&gt;</code> does not support public transfer, use this function to lock an object to an address.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock_and_transfer">lock_and_transfer</a>&lt;T: store&gt;(obj: T, <b>to</b>: <b>address</b>, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock_and_transfer">lock_and_transfer</a>&lt;T: store&gt;(
    obj: T,
    <b>to</b>: <b>address</b>,
    expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext
) {
    <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(<a href="../iota-framework/timelock.md#0x2_timelock_lock">lock</a>(obj, expiration_timestamp_ms, ctx), <b>to</b>);
}
</code></pre>



</details>

<a name="0x2_timelock_lock_with_label_and_transfer"></a>

## Function `lock_with_label_and_transfer`

Function to lock a labeled object <code>obj</code> until <code>expiration_timestamp_ms</code> and transfer it to address <code><b>to</b></code>.
Since <code>Timelock&lt;T&gt;</code> does not support public transfer, use this function to lock a labeled object to an address.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock_with_label_and_transfer">lock_with_label_and_transfer</a>&lt;T: store, L&gt;(<a href="../iota-framework/labeler.md#0x2_labeler">labeler</a>: &<a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;, obj: T, <b>to</b>: <b>address</b>, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_lock_with_label_and_transfer">lock_with_label_and_transfer</a>&lt;T: store, L&gt;(
    <a href="../iota-framework/labeler.md#0x2_labeler">labeler</a>: &LabelerCap&lt;L&gt;,
    obj: T,
    <b>to</b>: <b>address</b>,
    expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    ctx: &<b>mut</b> TxContext
) {
    <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(<a href="../iota-framework/timelock.md#0x2_timelock_lock_with_label">lock_with_label</a>(<a href="../iota-framework/labeler.md#0x2_labeler">labeler</a>, obj, expiration_timestamp_ms, ctx), <b>to</b>);
}
</code></pre>



</details>

<a name="0x2_timelock_unlock"></a>

## Function `unlock`

Function to unlock the object from a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_unlock">unlock</a>&lt;T: store&gt;(self: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): T
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_unlock">unlock</a>&lt;T: store&gt;(self: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext): T {
    // Unpack the <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <b>let</b> (locked, expiration_timestamp_ms, _) = <a href="../iota-framework/timelock.md#0x2_timelock_unpack">unpack</a>(self);

    // Check <b>if</b> the lock <b>has</b> expired.
    <b>assert</b>!(<a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a> &lt;= ctx.epoch_timestamp_ms(), <a href="../iota-framework/timelock.md#0x2_timelock_ENotExpiredYet">ENotExpiredYet</a>);

    locked
}
</code></pre>



</details>

<a name="0x2_timelock_join"></a>

## Function `join`

Join two <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;</code> together.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_join">join</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, other: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_join">join</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;, other: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;) {
    // Check the preconditions.
    <b>assert</b>!(self.<a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>() == other.<a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>(), <a href="../iota-framework/timelock.md#0x2_timelock_EDifferentExpirationTime">EDifferentExpirationTime</a>);
    <b>assert</b>!(self.<a href="../iota-framework/timelock.md#0x2_timelock_label">label</a>() == other.<a href="../iota-framework/timelock.md#0x2_timelock_label">label</a>(), <a href="../iota-framework/timelock.md#0x2_timelock_EDifferentLabels">EDifferentLabels</a>);

    // Unpack the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (value, _, _) = <a href="../iota-framework/timelock.md#0x2_timelock_unpack">unpack</a>(other);

    // Join the balances.
    self.locked.<a href="../iota-framework/timelock.md#0x2_timelock_join">join</a>(value);
}
</code></pre>



</details>

<a name="0x2_timelock_join_vec"></a>

## Function `join_vec`

Join everything in <code>others</code> with <code>self</code>.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_join_vec">join_vec</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, others: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_join_vec">join_vec</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;, <b>mut</b> others: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;&gt;) {
    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, others.length());

    // Join all the balances.
    <b>while</b> (i &lt; len) {
        <b>let</b> other = others.pop_back();
        <a href="../iota-framework/timelock.md#0x2_timelock_join">Self::join</a>(self, other);
        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(others)
}
</code></pre>



</details>

<a name="0x2_timelock_split"></a>

## Function `split`

Split a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;</code> and take a sub balance from it.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_split">split</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_split">split</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt; {
    // Split the locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> value = self.locked.<a href="../iota-framework/timelock.md#0x2_timelock_split">split</a>(value);

    // Pack the split <a href="../iota-framework/balance.md#0x2_balance">balance</a> into a <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <a href="../iota-framework/timelock.md#0x2_timelock_pack">pack</a>(value, self.<a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>(), self.<a href="../iota-framework/timelock.md#0x2_timelock_label">label</a>(), ctx)
}
</code></pre>



</details>

<a name="0x2_timelock_split_balance"></a>

## Function `split_balance`

Split the given <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;</code> into two parts, one with principal <code>value</code>,
and transfer the newly split part to the sender address.


<pre><code><b>public</b> entry <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_split_balance">split_balance</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_split_balance">split_balance</a>&lt;T&gt;(self: &<b>mut</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;Balance&lt;T&gt;&gt;, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext) {
    <a href="../iota-framework/timelock.md#0x2_timelock_split">split</a>(self, value, ctx).<a href="../iota-framework/timelock.md#0x2_timelock_transfer_to_sender">transfer_to_sender</a>(ctx)
}
</code></pre>



</details>

<a name="0x2_timelock_transfer_to_sender"></a>

## Function `transfer_to_sender`

A utility function to transfer a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> to the sender.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_transfer_to_sender">transfer_to_sender</a>&lt;T: store&gt;(lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_transfer_to_sender">transfer_to_sender</a>&lt;T: store&gt;(lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext) {
    <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(lock, ctx.sender())
}
</code></pre>



</details>

<a name="0x2_timelock_system_pack"></a>

## Function `system_pack`

A utility function to pack a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> that can be invoked only by a system package.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_system_pack">system_pack</a>&lt;T: store&gt;(_: &<a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">timelock::SystemTimelockCap</a>, locked: T, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_system_pack">system_pack</a>&lt;T: store&gt;(
    _: &<a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a>,
    locked: T,
    expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    label: Option&lt;String&gt;,
    ctx: &<b>mut</b> TxContext): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;
{
    <a href="../iota-framework/timelock.md#0x2_timelock_pack">pack</a>(locked, expiration_timestamp_ms, label, ctx)
}
</code></pre>



</details>

<a name="0x2_timelock_system_unpack"></a>

## Function `system_unpack`

An utility function to unpack a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> that can be invoked only by a system package.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_system_unpack">system_unpack</a>&lt;T: store&gt;(_: &<a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">timelock::SystemTimelockCap</a>, lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): (T, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_system_unpack">system_unpack</a>&lt;T: store&gt;(_: &<a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a>, lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;): (T, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, Option&lt;String&gt;) {
    <a href="../iota-framework/timelock.md#0x2_timelock_unpack">unpack</a>(lock)
}
</code></pre>



</details>

<a name="0x2_timelock_type_name"></a>

## Function `type_name`

Return a fully qualified type name with the original package IDs
that is used as type related a label value.


<pre><code><b>public</b> <b>fun</b> <a href="../move-stdlib/type_name.md#0x1_type_name">type_name</a>&lt;L&gt;(): <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../move-stdlib/type_name.md#0x1_type_name">type_name</a>&lt;L&gt;(): String {
    <a href="../move-stdlib/string.md#0x1_string_from_ascii">string::from_ascii</a>(std::type_name::get_with_original_ids&lt;L&gt;().into_string())
}
</code></pre>



</details>

<a name="0x2_timelock_expiration_timestamp_ms"></a>

## Function `expiration_timestamp_ms`

Function to get the expiration timestamp of a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    self.expiration_timestamp_ms
}
</code></pre>



</details>

<a name="0x2_timelock_is_locked"></a>

## Function `is_locked`

Function to check if a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> is locked.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_is_locked">is_locked</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_is_locked">is_locked</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext): bool {
    self.<a href="../iota-framework/timelock.md#0x2_timelock_remaining_time">remaining_time</a>(ctx) &gt; 0
}
</code></pre>



</details>

<a name="0x2_timelock_remaining_time"></a>

## Function `remaining_time`

Function to get the remaining time of a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.
Returns 0 if the lock has expired.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_remaining_time">remaining_time</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_remaining_time">remaining_time</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    // Get the epoch timestamp.
    <b>let</b> current_timestamp_ms = ctx.epoch_timestamp_ms();

    // Check <b>if</b> the lock <b>has</b> expired.
    <b>if</b> (self.<a href="../iota-framework/timelock.md#0x2_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a> &lt; current_timestamp_ms) {
        <b>return</b> 0
    };

    // Calculate the remaining time.
    self.expiration_timestamp_ms - current_timestamp_ms
}
</code></pre>



</details>

<a name="0x2_timelock_locked"></a>

## Function `locked`

Function to get the locked object of a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_locked">locked</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): &T
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_locked">locked</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;): &T {
    &self.locked
}
</code></pre>



</details>

<a name="0x2_timelock_label"></a>

## Function `label`

Function to get the label of a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_label">label</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_label">label</a>&lt;T: store&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;): Option&lt;String&gt; {
    self.label
}
</code></pre>



</details>

<a name="0x2_timelock_is_labeled_with"></a>

## Function `is_labeled_with`

Check if a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> is labeled with the type <code>L</code>.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_is_labeled_with">is_labeled_with</a>&lt;T: store, L&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_is_labeled_with">is_labeled_with</a>&lt;T: store, L&gt;(self: &<a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;): bool {
    <b>if</b> (self.label.is_some()) {
        self.label.borrow() == <a href="../move-stdlib/type_name.md#0x1_type_name">type_name</a>&lt;L&gt;()
    }
    <b>else</b> {
        <b>false</b>
    }
}
</code></pre>



</details>

<a name="0x2_timelock_pack"></a>

## Function `pack`

A utility function to pack a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_pack">pack</a>&lt;T: store&gt;(locked: T, expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_pack">pack</a>&lt;T: store&gt;(
    locked: T,
    expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>,
    label: Option&lt;String&gt;,
    ctx: &<b>mut</b> TxContext): <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;
{
    // Create a <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        locked,
        expiration_timestamp_ms,
        label,
    }
}
</code></pre>



</details>

<a name="0x2_timelock_unpack"></a>

## Function `unpack`

An utility function to unpack a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_unpack">unpack</a>&lt;T: store&gt;(lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): (T, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_unpack">unpack</a>&lt;T: store&gt;(lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;): (T, <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, Option&lt;String&gt;) {
    // Unpack the <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <b>let</b> <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a> {
        id,
        locked,
        expiration_timestamp_ms,
        label,
    } = lock;

    // Delete the <a href="../iota-framework/timelock.md#0x2_timelock">timelock</a>.
    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);

    (locked, expiration_timestamp_ms, label)
}
</code></pre>



</details>

<a name="0x2_timelock_transfer"></a>

## Function `transfer`

A utility function to transfer a <code><a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a></code> to a receiver.


<pre><code><b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>&lt;T: store&gt;(lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, receiver: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>&lt;T: store&gt;(lock: <a href="../iota-framework/timelock.md#0x2_timelock_TimeLock">TimeLock</a>&lt;T&gt;, receiver: <b>address</b>) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(lock, receiver);
}
</code></pre>



</details>

<a name="0x2_timelock_check_expiration_timestamp_ms"></a>

## Function `check_expiration_timestamp_ms`

An utility function to check that the <code>expiration_timestamp_ms</code> value is valid.


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &TxContext) {
    // Get the epoch timestamp.
    <b>let</b> epoch_timestamp_ms = ctx.epoch_timestamp_ms();

    // Check that `expiration_timestamp_ms` is valid.
    <b>assert</b>!(expiration_timestamp_ms &gt; epoch_timestamp_ms, <a href="../iota-framework/timelock.md#0x2_timelock_EExpireEpochIsPast">EExpireEpochIsPast</a>);
}
</code></pre>



</details>

<a name="0x2_timelock_new_system_timelock_cap"></a>

## Function `new_system_timelock_cap`

Create a <code><a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a></code>.
This should be called only once during genesis creation.


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_new_system_timelock_cap">new_system_timelock_cap</a>(ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">timelock::SystemTimelockCap</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/timelock.md#0x2_timelock_new_system_timelock_cap">new_system_timelock_cap</a>(ctx: &TxContext): <a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a> {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/timelock.md#0x2_timelock_ENotSystemAddress">ENotSystemAddress</a>);
    <b>assert</b>!(ctx.epoch() == 0, <a href="../iota-framework/timelock.md#0x2_timelock_ENotCalledAtGenesis">ENotCalledAtGenesis</a>);

    <a href="../iota-framework/timelock.md#0x2_timelock_SystemTimelockCap">SystemTimelockCap</a> {}
}
</code></pre>



</details>
