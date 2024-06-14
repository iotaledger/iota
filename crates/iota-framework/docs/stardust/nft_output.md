---
title: Module `0x107a::nft_output`
---



-  [Resource `NftOutput`](#0x107a_nft_output_NftOutput)
-  [Constants](#@Constants_0)
-  [Function `extract_assets`](#0x107a_nft_output_extract_assets)
-  [Function `load_nft`](#0x107a_nft_output_load_nft)
-  [Function `attach_nft`](#0x107a_nft_output_attach_nft)
-  [Function `receive`](#0x107a_nft_output_receive)


<pre><code><b>use</b> <a href="expiration_unlock_condition.md#0x107a_expiration_unlock_condition">0x107a::expiration_unlock_condition</a>;
<b>use</b> <a href="nft.md#0x107a_nft">0x107a::nft</a>;
<b>use</b> <a href="storage_deposit_return_unlock_condition.md#0x107a_storage_deposit_return_unlock_condition">0x107a::storage_deposit_return_unlock_condition</a>;
<b>use</b> <a href="timelock_unlock_condition.md#0x107a_timelock_unlock_condition">0x107a::timelock_unlock_condition</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../iota-framework/bag.md#0x2_bag">0x2::bag</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/dynamic_object_field.md#0x2_dynamic_object_field">0x2::dynamic_object_field</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x107a_nft_output_NftOutput"></a>

## Resource `NftOutput`

The Stardust NFT output representation.


<pre><code><b>struct</b> <a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a>&lt;T&gt; <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>
 This is a "random" UID, not the NFTID from Stardust.
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
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x107a_nft_output_NFT_NAME"></a>

The NFT dynamic field name.


<pre><code><b>const</b> <a href="nft_output.md#0x107a_nft_output_NFT_NAME">NFT_NAME</a>: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt; = [110, 102, 116];
</code></pre>



<a name="0x107a_nft_output_extract_assets"></a>

## Function `extract_assets`

The function extracts assets from a legacy NFT output.


<pre><code><b>public</b> <b>fun</b> <a href="nft_output.md#0x107a_nft_output_extract_assets">extract_assets</a>&lt;T&gt;(output: <a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&lt;T&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): (<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;, <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>, <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft_output.md#0x107a_nft_output_extract_assets">extract_assets</a>&lt;T&gt;(<b>mut</b> output: <a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a>&lt;T&gt;, ctx: &<b>mut</b> TxContext): (Balance&lt;T&gt;, Bag, Nft) {
    // Load the related Nft <a href="../iota-framework/object.md#0x2_object">object</a>.
    <b>let</b> <a href="nft.md#0x107a_nft">nft</a> = <a href="nft_output.md#0x107a_nft_output_load_nft">load_nft</a>(&<b>mut</b> output);

    // Unpuck the output.
    <b>let</b> <a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a> {
        id,
        <a href="../iota-framework/balance.md#0x2_balance">balance</a>: <b>mut</b> <a href="../iota-framework/balance.md#0x2_balance">balance</a>,
        native_tokens,
        storage_deposit_return_uc: <b>mut</b> storage_deposit_return_uc,
        timelock_uc: <b>mut</b> timelock_uc,
        expiration_uc: <b>mut</b> expiration_uc
    } = output;

    // If the output <b>has</b> a timelock unlock condition, then we need <b>to</b> check <b>if</b> the timelock_uc <b>has</b> expired.
    <b>if</b> (timelock_uc.is_some()) {
        timelock_uc.extract().unlock(ctx);
    };

    // If the output <b>has</b> an expiration unlock condition, then we need <b>to</b> check who can unlock the output.
    <b>if</b> (expiration_uc.is_some()) {
        expiration_uc.extract().unlock(ctx);
    };

    // If the output <b>has</b> a storage deposit <b>return</b> unlock condition, then we need <b>to</b> <b>return</b> the deposit.
    <b>if</b> (storage_deposit_return_uc.is_some()) {
        storage_deposit_return_uc.extract().unlock(&<b>mut</b> <a href="../iota-framework/balance.md#0x2_balance">balance</a>, ctx);
    };

    // Destroy the output.
    <a href="../move-stdlib/option.md#0x1_option_destroy_none">option::destroy_none</a>(timelock_uc);
    <a href="../move-stdlib/option.md#0x1_option_destroy_none">option::destroy_none</a>(expiration_uc);
    <a href="../move-stdlib/option.md#0x1_option_destroy_none">option::destroy_none</a>(storage_deposit_return_uc);

    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);

    <b>return</b> (<a href="../iota-framework/balance.md#0x2_balance">balance</a>, native_tokens, <a href="nft.md#0x107a_nft">nft</a>)
}
</code></pre>



</details>

<a name="0x107a_nft_output_load_nft"></a>

## Function `load_nft`

Loads the related <code>Nft</code> object.


<pre><code><b>fun</b> <a href="nft_output.md#0x107a_nft_output_load_nft">load_nft</a>&lt;T&gt;(output: &<b>mut</b> <a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&lt;T&gt;): <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="nft_output.md#0x107a_nft_output_load_nft">load_nft</a>&lt;T&gt;(output: &<b>mut</b> <a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a>&lt;T&gt;): Nft {
    <a href="../iota-framework/dynamic_object_field.md#0x2_dynamic_object_field_remove">dynamic_object_field::remove</a>(&<b>mut</b> output.id, <a href="nft_output.md#0x107a_nft_output_NFT_NAME">NFT_NAME</a>)
}
</code></pre>



</details>

<a name="0x107a_nft_output_attach_nft"></a>

## Function `attach_nft`

Utility function to attach an <code>Alias</code> to an <code>AliasOutput</code>.


<pre><code><b>public</b> <b>fun</b> <a href="nft_output.md#0x107a_nft_output_attach_nft">attach_nft</a>&lt;T&gt;(output: &<b>mut</b> <a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&lt;T&gt;, <a href="nft.md#0x107a_nft">nft</a>: <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft_output.md#0x107a_nft_output_attach_nft">attach_nft</a>&lt;T&gt;(output: &<b>mut</b> <a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a>&lt;T&gt;, <a href="nft.md#0x107a_nft">nft</a>: Nft) {
    <a href="../iota-framework/dynamic_object_field.md#0x2_dynamic_object_field_add">dynamic_object_field::add</a>(&<b>mut</b> output.id, <a href="nft_output.md#0x107a_nft_output_NFT_NAME">NFT_NAME</a>, <a href="nft.md#0x107a_nft">nft</a>)
}
</code></pre>



</details>

<a name="0x107a_nft_output_receive"></a>

## Function `receive`

Utility function to receive an <code><a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a></code> in other Stardust modules.
Other modules in the stardust package can call this function to receive an <code><a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a></code> (alias).


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="nft_output.md#0x107a_nft_output_receive">receive</a>&lt;T&gt;(parent: &<b>mut</b> <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a>, <a href="nft.md#0x107a_nft">nft</a>: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&lt;T&gt;&gt;): <a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<a href="../iota-framework/package.md#0x2_package">package</a>) <b>fun</b> <a href="nft_output.md#0x107a_nft_output_receive">receive</a>&lt;T&gt;(parent: &<b>mut</b> UID, <a href="nft.md#0x107a_nft">nft</a>: Receiving&lt;<a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a>&lt;T&gt;&gt;) : <a href="nft_output.md#0x107a_nft_output_NftOutput">NftOutput</a>&lt;T&gt; {
    <a href="../iota-framework/transfer.md#0x2_transfer_receive">transfer::receive</a>(parent, <a href="nft.md#0x107a_nft">nft</a>)
}
</code></pre>



</details>
