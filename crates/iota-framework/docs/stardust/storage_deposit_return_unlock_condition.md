---
title: Module `0x107a::storage_deposit_return_unlock_condition`
---



-  [Struct `StorageDepositReturnUnlockCondition`](#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition)
-  [Function `unlock`](#0x107a_storage_deposit_return_unlock_condition_unlock)
-  [Function `return_address`](#0x107a_storage_deposit_return_unlock_condition_return_address)
-  [Function `return_amount`](#0x107a_storage_deposit_return_unlock_condition_return_amount)


<pre><code><b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition"></a>

## Struct `StorageDepositReturnUnlockCondition`

The Stardust storage deposit return unlock condition.


<pre><code><b>struct</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">StorageDepositReturnUnlockCondition</a> <b>has</b> store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>return_address: <b>address</b></code>
</dt>
<dd>
 The address to which the consuming transaction should deposit the amount defined in Return Amount.
</dd>
<dt>
<code>return_amount: <a href="../move-stdlib/u64.md#0x1_u64">u64</a></code>
</dt>
<dd>
 The amount of coins the consuming transaction should deposit to the address defined in Return Address.
</dd>
</dl>


</details>

<a name="0x107a_storage_deposit_return_unlock_condition_unlock"></a>

## Function `unlock`

Check the unlock condition.


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_unlock">unlock</a>&lt;T&gt;(condition: <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition</a>, funding: &<b>mut</b> <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_unlock">unlock</a>&lt;T&gt;(condition: <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">StorageDepositReturnUnlockCondition</a>, funding: &<b>mut</b> Balance&lt;T&gt;, ctx: &<b>mut</b> TxContext) {
    // Aborts <b>if</b> `funding` is not enough.
    <b>let</b> return_balance = funding.split(condition.<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_return_amount">return_amount</a>());

    // Recipient will need <b>to</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a> the <a href="../iota-framework/coin.md#0x2_coin">coin</a> <b>to</b> a normal ed25519 <b>address</b> instead of legacy.
    public_transfer(from_balance(return_balance, ctx), condition.<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_return_address">return_address</a>());

    <b>let</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">StorageDepositReturnUnlockCondition</a> {
        return_address: _,
        return_amount: _,
    } = condition;
}
</code></pre>



</details>

<a name="0x107a_storage_deposit_return_unlock_condition_return_address"></a>

## Function `return_address`

Get the unlock condition's <code>return_address</code>.


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_return_address">return_address</a>(condition: &<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition</a>): <b>address</b>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_return_address">return_address</a>(condition: &<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">StorageDepositReturnUnlockCondition</a>): <b>address</b> {
    condition.return_address
}
</code></pre>



</details>

<a name="0x107a_storage_deposit_return_unlock_condition_return_amount"></a>

## Function `return_amount`

Get the unlock condition's <code>return_amount</code>.


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_return_amount">return_amount</a>(condition: &<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_return_amount">return_amount</a>(condition: &<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">StorageDepositReturnUnlockCondition</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    condition.return_amount
}
</code></pre>



</details>
