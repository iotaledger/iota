---
title: Module `0x107a::basic_output`
---



-  [Resource `BasicOutput`](#0x107a_basic_output_BasicOutput)
-  [Function `extract_assets`](#0x107a_basic_output_extract_assets)
-  [Function `receive`](#0x107a_basic_output_receive)


<pre><code><b>use</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition">0x107a::expiration_unlock_condition</a>;
<b>use</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition">0x107a::storage_deposit_return_unlock_condition</a>;
<b>use</b> <a href="timelock_unlock_condition.md#0x107a_timelock_unlock_condition">0x107a::timelock_unlock_condition</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../iota-framework/bag.md#0x2_bag">0x2::bag</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x107a_basic_output_BasicOutput"></a>

## Resource `BasicOutput`

A basic output that has unlock conditions/features.
- basic outputs with expiration unlock condition must be a shared object, since that's the only
way to handle the two possible addresses that can unlock the output.
- notice that there is no <code>store</code> ability and there is no custom transfer function:
-  you can call <code>extract_assets</code>,
-  or you can call <code>receive</code> in other models to receive a <code><a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a></code>.


<pre><code><b>struct</b> <a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a>&lt;T&gt; <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>
 Hash of the <code>outputId</code> that was migrated.
</dd>
<dt>
<code><a href="../iota-framework/balance.md#0x2_balance">balance</a>: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;</code>
</dt>
<dd>
 The amount of coins held by the output.
</dd>
<dt>
<code>native_tokens: <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a></code>
</dt>
<dd>
 The <code>Bag</code> holds native tokens, key-ed by the stringified type of the asset.
 Example: key: "0xabcded::soon::SOON", value: Balance<0xabcded::soon::SOON>.
</dd>
<dt>
<code>storage_deposit_return_uc: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition_StorageDepositReturnUnlockCondition">storage_deposit_return_unlock_condition::StorageDepositReturnUnlockCondition</a>&gt;</code>
</dt>
<dd>
 The storage deposit return unlock condition.
</dd>
<dt>
<code>timelock_uc: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="timelock_unlock_condition.md#0x107a_timelock_unlock_condition_TimelockUnlockCondition">timelock_unlock_condition::TimelockUnlockCondition</a>&gt;</code>
</dt>
<dd>
 The timelock unlock condition.
</dd>
<dt>
<code>expiration_uc: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition_ExpirationUnlockCondition">expiration_unlock_condition::ExpirationUnlockCondition</a>&gt;</code>
</dt>
<dd>
 The expiration unlock condition.
</dd>
<dt>
<code>metadata: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 The metadata feature.
</dd>
<dt>
<code>tag: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 The tag feature.
</dd>
<dt>
<code>sender: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;</code>
</dt>
<dd>
 The sender feature.
</dd>
</dl>


</details>

<a name="0x107a_basic_output_extract_assets"></a>

## Function `extract_assets`

Extract the assets stored inside the output, respecting the unlock conditions.
- The object will be deleted.
- The <code>StorageDepositReturnUnlockCondition</code> will return the deposit.
- Remaining assets (coins and native tokens) will be returned.


<pre><code><b>public</b> <b>fun</b> <a href="basic_output.md#0x107a_basic_output_extract_assets">extract_assets</a>&lt;T&gt;(output: <a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>&lt;T&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): (<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;, <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="basic_output.md#0x107a_basic_output_extract_assets">extract_assets</a>&lt;T&gt;(output: <a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a>&lt;T&gt;, ctx: &<b>mut</b> TxContext) : (Balance&lt;T&gt;, Bag) {
    // Unpack the output into its basic part.
    <b>let</b> <a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a> {
        id,
        <a href="../iota-framework/balance.md#0x2_balance">balance</a>: <b>mut</b> <a href="../iota-framework/balance.md#0x2_balance">balance</a>,
        native_tokens,
        storage_deposit_return_uc: <b>mut</b> storage_deposit_return_uc,
        timelock_uc: <b>mut</b> timelock_uc,
        expiration_uc: <b>mut</b> expiration_uc,
        sender: _,
        metadata: _,
        tag: _
    } = output;

    // If the output <b>has</b> a timelock unlock condition, then we need <b>to</b> check <b>if</b> the timelock_uc <b>has</b> expired.
    <b>if</b> (timelock_uc.is_some()) {
        timelock_uc.extract().unlock(ctx);
    };

    // If the output <b>has</b> an expiration unlock condition, then we need <b>to</b> check who can unlock the output.
    <b>if</b> (expiration_uc.is_some()) {
        expiration_uc.extract().unlock(ctx);
    };

    // If the output <b>has</b> an storage deposit <b>return</b> unlock condition, then we need <b>to</b> <b>return</b> the deposit.
    <b>if</b> (storage_deposit_return_uc.is_some()) {
        storage_deposit_return_uc.extract().unlock(&<b>mut</b> <a href="../iota-framework/balance.md#0x2_balance">balance</a>, ctx);
    };

    // Destroy the unlock conditions.
    <a href="../move-stdlib/option.md#0x1_option_destroy_none">option::destroy_none</a>(timelock_uc);
    <a href="../move-stdlib/option.md#0x1_option_destroy_none">option::destroy_none</a>(expiration_uc);
    <a href="../move-stdlib/option.md#0x1_option_destroy_none">option::destroy_none</a>(storage_deposit_return_uc);

    // Delete the output.
    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);

    <b>return</b> (<a href="../iota-framework/balance.md#0x2_balance">balance</a>, native_tokens)
}
</code></pre>



</details>

<a name="0x107a_basic_output_receive"></a>

## Function `receive`

Utility function to receive a basic output in other stardust modules.
Since <code><a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a></code> only has <code>key</code>, it can not be received via <code>public_receive</code>.
The private receiver must be implemented in its defining module (here).
Other modules in the Stardust package can call this function to receive a basic output (alias, NFT).


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="basic_output.md#0x107a_basic_output_receive">receive</a>&lt;T&gt;(parent: &<b>mut</b> <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a>, output: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>&lt;T&gt;&gt;): <a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<a href="../iota-framework/package.md#0x2_package">package</a>) <b>fun</b> <a href="basic_output.md#0x107a_basic_output_receive">receive</a>&lt;T&gt;(parent: &<b>mut</b> UID, output: Receiving&lt;<a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a>&lt;T&gt;&gt;) : <a href="basic_output.md#0x107a_basic_output_BasicOutput">BasicOutput</a>&lt;T&gt; {
    <a href="../iota-framework/transfer.md#0x2_transfer_receive">transfer::receive</a>(parent, output)
}
</code></pre>



</details>
