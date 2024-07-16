---
title: Module `0x107a::alias_output`
---



-  [Resource `AliasOutput`](#0x107a_alias_output_AliasOutput)
-  [Constants](#@Constants_0)
-  [Function `extract_assets`](#0x107a_alias_output_extract_assets)
-  [Function `receive`](#0x107a_alias_output_receive)
-  [Function `attach_alias`](#0x107a_alias_output_attach_alias)
-  [Function `load_alias`](#0x107a_alias_output_load_alias)


<pre><code><b>use</b> <a href="alias.md#0x107a_alias">0x107a::alias</a>;
<b>use</b> <a href="../iota-framework/bag.md#0x2_bag">0x2::bag</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/dynamic_object_field.md#0x2_dynamic_object_field">0x2::dynamic_object_field</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
</code></pre>



<a name="0x107a_alias_output_AliasOutput"></a>

## Resource `AliasOutput`

Owned Object controlled by the Governor Address.


<pre><code><b>struct</b> <a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a>&lt;T&gt; <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>
 This is a "random" UID, not the AliasID from Stardust.
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
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x107a_alias_output_ALIAS_NAME"></a>

The Alias dynamic object field name.


<pre><code><b>const</b> <a href="alias_output.md#0x107a_alias_output_ALIAS_NAME">ALIAS_NAME</a>: <a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt; = [97, 108, 105, 97, 115];
</code></pre>



<a name="0x107a_alias_output_extract_assets"></a>

## Function `extract_assets`

The function extracts assets from a legacy <code><a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a></code>.
- returns the coin Balance,
- the native tokens Bag,
- and the <code>Alias</code> object that persists the AliasID=ObjectID from Stardust.


<pre><code><b>public</b> <b>fun</b> <a href="alias_output.md#0x107a_alias_output_extract_assets">extract_assets</a>&lt;T&gt;(output: <a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&lt;T&gt;): (<a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;, <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>, <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias_output.md#0x107a_alias_output_extract_assets">extract_assets</a>&lt;T&gt;(<b>mut</b> output: <a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a>&lt;T&gt;): (Balance&lt;T&gt;, Bag, Alias) {
    // Load the related <a href="alias.md#0x107a_alias">alias</a> <a href="../iota-framework/object.md#0x2_object">object</a>.
    <b>let</b> <a href="alias.md#0x107a_alias">alias</a> = <a href="alias_output.md#0x107a_alias_output_load_alias">load_alias</a>(&<b>mut</b> output);

    // Unpack the output into its basic part.
    <b>let</b> <a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a> {
        id,
        <a href="../iota-framework/balance.md#0x2_balance">balance</a>,
        native_tokens
    } = output;

    // Delete the output.
    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);

    (<a href="../iota-framework/balance.md#0x2_balance">balance</a>, native_tokens, <a href="alias.md#0x107a_alias">alias</a>)
}
</code></pre>



</details>

<a name="0x107a_alias_output_receive"></a>

## Function `receive`

Utility function to receive an <code><a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a></code> object in other Stardust modules.
Other modules in the Stardust package can call this function to receive an <code><a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a></code> object (nft).


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="alias_output.md#0x107a_alias_output_receive">receive</a>&lt;T&gt;(parent: &<b>mut</b> <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a>, output: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&lt;T&gt;&gt;): <a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<a href="../iota-framework/package.md#0x2_package">package</a>) <b>fun</b> <a href="alias_output.md#0x107a_alias_output_receive">receive</a>&lt;T&gt;(parent: &<b>mut</b> UID, output: Receiving&lt;<a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a>&lt;T&gt;&gt;) : <a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a>&lt;T&gt; {
    <a href="../iota-framework/transfer.md#0x2_transfer_receive">transfer::receive</a>(parent, output)
}
</code></pre>



</details>

<a name="0x107a_alias_output_attach_alias"></a>

## Function `attach_alias`

Utility function to attach an <code>Alias</code> to an <code><a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias_output.md#0x107a_alias_output_attach_alias">attach_alias</a>&lt;T&gt;(output: &<b>mut</b> <a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&lt;T&gt;, <a href="alias.md#0x107a_alias">alias</a>: <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias_output.md#0x107a_alias_output_attach_alias">attach_alias</a>&lt;T&gt;(output: &<b>mut</b> <a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a>&lt;T&gt;, <a href="alias.md#0x107a_alias">alias</a>: Alias) {
    <a href="../iota-framework/dynamic_object_field.md#0x2_dynamic_object_field_add">dynamic_object_field::add</a>(&<b>mut</b> output.id, <a href="alias_output.md#0x107a_alias_output_ALIAS_NAME">ALIAS_NAME</a>, <a href="alias.md#0x107a_alias">alias</a>)
}
</code></pre>



</details>

<a name="0x107a_alias_output_load_alias"></a>

## Function `load_alias`

Loads the <code>Alias</code> object from the dynamic object field.


<pre><code><b>fun</b> <a href="alias_output.md#0x107a_alias_output_load_alias">load_alias</a>&lt;T&gt;(output: &<b>mut</b> <a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&lt;T&gt;): <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="alias_output.md#0x107a_alias_output_load_alias">load_alias</a>&lt;T&gt;(output: &<b>mut</b> <a href="alias_output.md#0x107a_alias_output_AliasOutput">AliasOutput</a>&lt;T&gt;): Alias {
    <a href="../iota-framework/dynamic_object_field.md#0x2_dynamic_object_field_remove">dynamic_object_field::remove</a>(&<b>mut</b> output.id, <a href="alias_output.md#0x107a_alias_output_ALIAS_NAME">ALIAS_NAME</a>)
}
</code></pre>



</details>
