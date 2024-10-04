---
title: Module `0x107a::expiration_unlock_condition`
---



-  [Struct `ExpirationUnlockCondition`](#0x107a_expiration_unlock_condition_ExpirationUnlockCondition)
-  [Constants](#@Constants_0)
-  [Function `unlock`](#0x107a_expiration_unlock_condition_unlock)
-  [Function `can_be_unlocked_by`](#0x107a_expiration_unlock_condition_can_be_unlocked_by)
-  [Function `owner`](#0x107a_expiration_unlock_condition_owner)
-  [Function `return_address`](#0x107a_expiration_unlock_condition_return_address)
-  [Function `unix_time`](#0x107a_expiration_unlock_condition_unix_time)


<pre><code><b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x107a_expiration_unlock_condition_ExpirationUnlockCondition"></a>

## Struct `ExpirationUnlockCondition`

The Stardust expiration unlock condition.


<pre><code><b>struct</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a> <b>has</b> store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>owner: <b>address</b></code>
</dt>
<dd>
 The address who owns the output before the timestamp has passed.
</dd>
<dt>
<code>return_address: <b>address</b></code>
</dt>
<dd>
 The address that is allowed to spend the locked funds after the timestamp has passed.
</dd>
<dt>
<code>unix_time: u32</code>
</dt>
<dd>
 Before this unix time, Address Unlock Condition is allowed to unlock the output, after that only the address defined in Return Address.
</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x107a_expiration_unlock_condition_EWrongSender"></a>

The output can not be unlocked by the sender error.


<pre><code><b>const</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_EWrongSender">EWrongSender</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x107a_expiration_unlock_condition_unlock"></a>

## Function `unlock`

Check the unlock condition.


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_unlock">unlock</a>(condition: <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">expiration_unlock_condition::ExpirationUnlockCondition</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_unlock">unlock</a>(condition: <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a>, ctx: &<b>mut</b> TxContext) {
    <b>let</b> unlock_address = condition.<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_can_be_unlocked_by">can_be_unlocked_by</a>(ctx);

    <b>assert</b>!(unlock_address == ctx.sender(), <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_EWrongSender">EWrongSender</a>);

    <b>let</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a> {
        owner: _,
        return_address: _,
        unix_time: _,
    } = condition;
}
</code></pre>



</details>

<a name="0x107a_expiration_unlock_condition_can_be_unlocked_by"></a>

## Function `can_be_unlocked_by`

Return the address that can unlock the related output.


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_can_be_unlocked_by">can_be_unlocked_by</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">expiration_unlock_condition::ExpirationUnlockCondition</a>, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <b>address</b>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_can_be_unlocked_by">can_be_unlocked_by</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a>, ctx: &TxContext): <b>address</b> {
    // Unix time in seconds.
    <b>let</b> current_time = ((<a href="../iota-framework/tx_context.md#0x2_tx_context_epoch_timestamp_ms">tx_context::epoch_timestamp_ms</a>(ctx) / 1000) <b>as</b> u32);

    <b>if</b> (condition.<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_unix_time">unix_time</a>() &lt;= current_time) {
        condition.<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_return_address">return_address</a>()
    } <b>else</b> {
        condition.<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_owner">owner</a>()
    }
}
</code></pre>



</details>

<a name="0x107a_expiration_unlock_condition_owner"></a>

## Function `owner`

Get the unlock condition's <code>owner</code>.


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_owner">owner</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">expiration_unlock_condition::ExpirationUnlockCondition</a>): <b>address</b>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_owner">owner</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a>): <b>address</b> {
    condition.owner
}
</code></pre>



</details>

<a name="0x107a_expiration_unlock_condition_return_address"></a>

## Function `return_address`

Get the unlock condition's <code>return_address</code>.


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_return_address">return_address</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">expiration_unlock_condition::ExpirationUnlockCondition</a>): <b>address</b>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_return_address">return_address</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a>): <b>address</b> {
    condition.return_address
}
</code></pre>



</details>

<a name="0x107a_expiration_unlock_condition_unix_time"></a>

## Function `unix_time`

Get the unlock condition's <code>unix_time</code>.


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_unix_time">unix_time</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">expiration_unlock_condition::ExpirationUnlockCondition</a>): u32
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_unix_time">unix_time</a>(condition: &<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">ExpirationUnlockCondition</a>): u32 {
    condition.unix_time
}
</code></pre>



</details>
