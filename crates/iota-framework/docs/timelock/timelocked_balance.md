---
title: Module `0x10cf::timelocked_balance`
---

Utility functions for time-locked balance.

-   [Constants](#@Constants_0)
-   [Function `join`](#0x10cf_timelocked_balance_join)
-   [Function `join_vec`](#0x10cf_timelocked_balance_join_vec)
-   [Function `split`](#0x10cf_timelocked_balance_split)
-   [Function `split_balance`](#0x10cf_timelocked_balance_split_balance)

<pre><code><b>use</b> <a href="timelock.md#0x10cf_timelock">0x10cf::timelock</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>

<a name="@Constants_0"></a>

## Constants

<a name="0x10cf_timelocked_balance_EDifferentExpirationTime"></a>

For when trying to join two time-locked balances with different expiration time.

<pre><code><b>const</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_EDifferentExpirationTime">EDifferentExpirationTime</a>: u64 = 0;
</code></pre>

<a name="0x10cf_timelocked_balance_EDifferentLabels"></a>

For when trying to join two time-locked balances with different labels.

<pre><code><b>const</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_EDifferentLabels">EDifferentLabels</a>: u64 = 1;
</code></pre>

<a name="0x10cf_timelocked_balance_join"></a>

## Function `join`

Join two <code>TimeLock&lt;Balance&lt;T&gt;&gt;</code> together.

<pre><code><b>public</b> <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_join">join</a>&lt;T&gt;(self: &<b>mut</b> <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, other: <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;)
</code></pre>

<details>
<summary>Implementation</summary>

<pre><code><b>public</b> <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_join">join</a>&lt;T&gt;(self: &<b>mut</b> TimeLock&lt;Balance&lt;T&gt;&gt;, other: TimeLock&lt;Balance&lt;T&gt;&gt;) {
    // Check the preconditions.
    <b>assert</b>!(self.expiration_timestamp_ms() == other.expiration_timestamp_ms(), <a href="timelocked_balance.md#0x10cf_timelocked_balance_EDifferentExpirationTime">EDifferentExpirationTime</a>);
    <b>assert</b>!(self.label() == other.label(), <a href="timelocked_balance.md#0x10cf_timelocked_balance_EDifferentLabels">EDifferentLabels</a>);

    // Unpack the time-locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> (value, _, _) = <a href="timelock.md#0x10cf_timelock_unpack">timelock::unpack</a>(other);

    // Join the balances.
    self.locked_mut().<a href="timelocked_balance.md#0x10cf_timelocked_balance_join">join</a>(value);
}
</code></pre>

</details>

<a name="0x10cf_timelocked_balance_join_vec"></a>

## Function `join_vec`

Join everything in <code>others</code> with <code>self</code>.

<pre><code><b>public</b> <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_join_vec">join_vec</a>&lt;T&gt;(self: &<b>mut</b> <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, others: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;&gt;)
</code></pre>

<details>
<summary>Implementation</summary>

<pre><code><b>public</b> <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_join_vec">join_vec</a>&lt;T&gt;(self: &<b>mut</b> TimeLock&lt;Balance&lt;T&gt;&gt;, <b>mut</b> others: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;TimeLock&lt;Balance&lt;T&gt;&gt;&gt;) {
    // Create useful variables.
    <b>let</b> (<b>mut</b> i, len) = (0, others.length());

    // Join all the balances.
    <b>while</b> (i &lt; len) {
        <b>let</b> other = others.pop_back();
        <a href="timelocked_balance.md#0x10cf_timelocked_balance_join">Self::join</a>(self, other);
        i = i + 1
    };

    // Destroy the empty <a href="../move-stdlib/vector.md#0x1_vector">vector</a>.
    <a href="../move-stdlib/vector.md#0x1_vector_destroy_empty">vector::destroy_empty</a>(others)
}
</code></pre>

</details>

<a name="0x10cf_timelocked_balance_split"></a>

## Function `split`

Split a <code>TimeLock&lt;Balance&lt;T&gt;&gt;</code> and take a sub balance from it.

<pre><code><b>public</b> <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_split">split</a>&lt;T&gt;(self: &<b>mut</b> <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, value: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;
</code></pre>

<details>
<summary>Implementation</summary>

<pre><code><b>public</b> <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_split">split</a>&lt;T&gt;(self: &<b>mut</b> TimeLock&lt;Balance&lt;T&gt;&gt;, value: u64, ctx: &<b>mut</b> TxContext): TimeLock&lt;Balance&lt;T&gt;&gt; {
    // Split the locked <a href="../iota-framework/balance.md#0x2_balance">balance</a>.
    <b>let</b> value = self.locked_mut().<a href="timelocked_balance.md#0x10cf_timelocked_balance_split">split</a>(value);

    // Pack the split <a href="../iota-framework/balance.md#0x2_balance">balance</a> into a <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <a href="timelock.md#0x10cf_timelock_pack">timelock::pack</a>(value, self.expiration_timestamp_ms(), self.label(), ctx)
}
</code></pre>

</details>

<a name="0x10cf_timelocked_balance_split_balance"></a>

## Function `split_balance`

Split the given <code>TimeLock&lt;Balance&lt;T&gt;&gt;</code> to the two parts, one with principal <code>value</code>,
transfer the newly split part to the sender address.

<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_split_balance">split_balance</a>&lt;T&gt;(stake: &<b>mut</b> <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;&gt;, value: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>

<details>
<summary>Implementation</summary>

<pre><code><b>public</b> entry <b>fun</b> <a href="timelocked_balance.md#0x10cf_timelocked_balance_split_balance">split_balance</a>&lt;T&gt;(stake: &<b>mut</b> TimeLock&lt;Balance&lt;T&gt;&gt;, value: u64, ctx: &<b>mut</b> TxContext) {
    <a href="timelocked_balance.md#0x10cf_timelocked_balance_split">split</a>(stake, value, ctx).self_transfer(ctx)
}
</code></pre>

</details>
