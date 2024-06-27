---
title: Module `0x3::storage_deposits`
---



-  [Struct `StorageDeposits`](#0x3_storage_deposits_StorageDeposits)
-  [Function `new`](#0x3_storage_deposits_new)
-  [Function `advance_epoch`](#0x3_storage_deposits_advance_epoch)
-  [Function `total_balance`](#0x3_storage_deposits_total_balance)


<pre><code><b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/iota.md#0x2_iota">0x2::iota</a>;
</code></pre>



<a name="0x3_storage_deposits_StorageDeposits"></a>

## Struct `StorageDeposits`

Struct representing the storage deposits fund, containing a <code>Balance</code>:
- <code>storage_balance</code> tracks the total balance of storage fees collected from transactions.


<pre><code><b>struct</b> <a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">StorageDeposits</a> <b>has</b> store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>refundable_balance: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;</code>
</dt>
<dd>

</dd>
<dt>
<code>non_refundable_balance: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x3_storage_deposits_new"></a>

## Function `new`

Called by <code><a href="iota_system.md#0x3_iota_system">iota_system</a></code> at genesis time.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="storage_deposits.md#0x3_storage_deposits_new">new</a>(initial_balance: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;): <a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">storage_deposits::StorageDeposits</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="storage_deposits.md#0x3_storage_deposits_new">new</a>(initial_balance: Balance&lt;IOTA&gt;) : <a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">StorageDeposits</a> {
    <a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">StorageDeposits</a> {
        // Initialize the storage deposits <a href="../iota-framework/balance.md#0x2_balance">balance</a>
        refundable_balance: initial_balance,
        non_refundable_balance: <a href="../iota-framework/balance.md#0x2_balance_zero">balance::zero</a>()
    }
}
</code></pre>



</details>

<a name="0x3_storage_deposits_advance_epoch"></a>

## Function `advance_epoch`

Called by <code><a href="iota_system.md#0x3_iota_system">iota_system</a></code> at epoch change times to process the inflows and outflows of storage deposits.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="storage_deposits.md#0x3_storage_deposits_advance_epoch">advance_epoch</a>(self: &<b>mut</b> <a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">storage_deposits::StorageDeposits</a>, storage_charges: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, storage_rebate_amount: u64): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="storage_deposits.md#0x3_storage_deposits_advance_epoch">advance_epoch</a>(
    self: &<b>mut</b> <a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">StorageDeposits</a>,
    storage_charges: Balance&lt;IOTA&gt;,
    storage_rebate_amount: u64,
) : Balance&lt;IOTA&gt; {
    self.refundable_balance.join(storage_charges);

    <b>let</b> storage_rebate = self.refundable_balance.split(storage_rebate_amount);

    //TODO: possibly mint and burn tokens here
    // mint_iota(treasury_cap, storage_charges.value(), ctx);
    // burn_iota(treasury_cap, storage_rebate_amount, ctx);
    storage_rebate
}
</code></pre>



</details>

<a name="0x3_storage_deposits_total_balance"></a>

## Function `total_balance`



<pre><code><b>public</b> <b>fun</b> <a href="storage_deposits.md#0x3_storage_deposits_total_balance">total_balance</a>(self: &<a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">storage_deposits::StorageDeposits</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="storage_deposits.md#0x3_storage_deposits_total_balance">total_balance</a>(self: &<a href="storage_deposits.md#0x3_storage_deposits_StorageDeposits">StorageDeposits</a>): u64 {
    self.refundable_balance.value() + self.non_refundable_balance.value()
}
</code></pre>



</details>
