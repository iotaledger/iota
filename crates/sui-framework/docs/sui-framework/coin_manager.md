---
title: Module `0x2::coin_manager`
---

The purpose of a CoinManager is to allow access to all
properties of a Coin on-chain from within a single shared object
This includes access to the total supply and metadata
In addition a optional maximum supply can be set and a custom
additional Metadata field can be added.


-  [Resource `CoinManager`](#0x2_coin_manager_CoinManager)
-  [Resource `CoinManagerCap`](#0x2_coin_manager_CoinManagerCap)
-  [Struct `CoinManaged`](#0x2_coin_manager_CoinManaged)
-  [Struct `OwnershipRenounced`](#0x2_coin_manager_OwnershipRenounced)
-  [Constants](#@Constants_0)
-  [Function `new`](#0x2_coin_manager_new)
-  [Function `add_additional_metadata`](#0x2_coin_manager_add_additional_metadata)
-  [Function `replace_additional_metadata`](#0x2_coin_manager_replace_additional_metadata)
-  [Function `additional_metadata`](#0x2_coin_manager_additional_metadata)
-  [Function `enforce_maximum_supply`](#0x2_coin_manager_enforce_maximum_supply)
-  [Function `renounce_ownership`](#0x2_coin_manager_renounce_ownership)
-  [Function `is_immutable`](#0x2_coin_manager_is_immutable)
-  [Function `metadata`](#0x2_coin_manager_metadata)
-  [Function `total_supply`](#0x2_coin_manager_total_supply)
-  [Function `maximum_supply`](#0x2_coin_manager_maximum_supply)
-  [Function `available_supply`](#0x2_coin_manager_available_supply)
-  [Function `has_maximum_supply`](#0x2_coin_manager_has_maximum_supply)
-  [Function `supply_immut`](#0x2_coin_manager_supply_immut)
-  [Function `mint`](#0x2_coin_manager_mint)
-  [Function `mint_balance`](#0x2_coin_manager_mint_balance)
-  [Function `burn`](#0x2_coin_manager_burn)
-  [Function `mint_and_transfer`](#0x2_coin_manager_mint_and_transfer)
-  [Function `update_name`](#0x2_coin_manager_update_name)
-  [Function `update_symbol`](#0x2_coin_manager_update_symbol)
-  [Function `update_description`](#0x2_coin_manager_update_description)
-  [Function `update_icon_url`](#0x2_coin_manager_update_icon_url)
-  [Function `decimals`](#0x2_coin_manager_decimals)
-  [Function `name`](#0x2_coin_manager_name)
-  [Function `symbol`](#0x2_coin_manager_symbol)
-  [Function `description`](#0x2_coin_manager_description)
-  [Function `icon_url`](#0x2_coin_manager_icon_url)


<pre><code><b>use</b> <a href="../move-stdlib/ascii.md#0x1_ascii">0x1::ascii</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../move-stdlib/type_name.md#0x1_type_name">0x1::type_name</a>;
<b>use</b> <a href="balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="dynamic_field.md#0x2_dynamic_field">0x2::dynamic_field</a>;
<b>use</b> <a href="event.md#0x2_event">0x2::event</a>;
<b>use</b> <a href="object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="url.md#0x2_url">0x2::url</a>;
</code></pre>



<a name="0x2_coin_manager_CoinManager"></a>

## Resource `CoinManager`

Holds all related objects to a Coin in a convenient shared function


<pre><code><b>struct</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt; <b>has</b> store, key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>

</dd>
<dt>
<code>treasury_cap: <a href="coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;T&gt;</code>
</dt>
<dd>

</dd>
<dt>
<code>metadata: <a href="coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;</code>
</dt>
<dd>

</dd>
<dt>
<code>maximum_supply: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;u64&gt;</code>
</dt>
<dd>

</dd>
<dt>
<code>ownership_renounced: bool</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x2_coin_manager_CoinManagerCap"></a>

## Resource `CoinManagerCap`

Like <code>TreasuryCap</code>, but for dealing with <code>TreasuryCap</code> inside <code><a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a></code> objects


<pre><code><b>struct</b> <a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt; <b>has</b> store, key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x2_coin_manager_CoinManaged"></a>

## Struct `CoinManaged`



<pre><code><b>struct</b> <a href="coin_manager.md#0x2_coin_manager_CoinManaged">CoinManaged</a> <b>has</b> <b>copy</b>, drop
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>coin_name: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="0x2_coin_manager_OwnershipRenounced"></a>

## Struct `OwnershipRenounced`



<pre><code><b>struct</b> <a href="coin_manager.md#0x2_coin_manager_OwnershipRenounced">OwnershipRenounced</a> <b>has</b> <b>copy</b>, drop
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>coin_name: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x2_coin_manager_EAdditionalMetadataAlreadyExists"></a>



<pre><code><b>const</b> <a href="coin_manager.md#0x2_coin_manager_EAdditionalMetadataAlreadyExists">EAdditionalMetadataAlreadyExists</a>: u64 = 2;
</code></pre>



<a name="0x2_coin_manager_EAdditionalMetadataDoesNotExist"></a>



<pre><code><b>const</b> <a href="coin_manager.md#0x2_coin_manager_EAdditionalMetadataDoesNotExist">EAdditionalMetadataDoesNotExist</a>: u64 = 3;
</code></pre>



<a name="0x2_coin_manager_EMaximumSupplyAlreadySet"></a>



<pre><code><b>const</b> <a href="coin_manager.md#0x2_coin_manager_EMaximumSupplyAlreadySet">EMaximumSupplyAlreadySet</a>: u64 = 1;
</code></pre>



<a name="0x2_coin_manager_EMaximumSupplyReached"></a>

The error returned when the maximum supply reached.


<pre><code><b>const</b> <a href="coin_manager.md#0x2_coin_manager_EMaximumSupplyReached">EMaximumSupplyReached</a>: u64 = 0;
</code></pre>



<a name="0x2_coin_manager_new"></a>

## Function `new`

Wraps all important objects related to a <code>Coin</code> inside a shared object


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_new">new</a>&lt;T&gt;(treasury_cap: <a href="coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;T&gt;, metadata: <a href="coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;, ctx: &<b>mut</b> <a href="tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): (<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_new">new</a>&lt;T&gt; (
    treasury_cap: TreasuryCap&lt;T&gt;,
    metadata: CoinMetadata&lt;T&gt;,
    ctx: &<b>mut</b> TxContext,
): (<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;, <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;) {

    <b>let</b> manager = <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a> {
        id: <a href="object.md#0x2_object_new">object::new</a>(ctx),
        treasury_cap,
        metadata,
        maximum_supply: <a href="../move-stdlib/option.md#0x1_option_none">option::none</a>(),
        ownership_renounced: <b>false</b>
    };

    <a href="event.md#0x2_event_emit">event::emit</a>(<a href="coin_manager.md#0x2_coin_manager_CoinManaged">CoinManaged</a> {
        coin_name: <a href="../move-stdlib/type_name.md#0x1_type_name_into_string">type_name::into_string</a>(<a href="../move-stdlib/type_name.md#0x1_type_name_get">type_name::get</a>&lt;T&gt;())
    });

    (
        <a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt; {
            id: <a href="object.md#0x2_object_new">object::new</a>(ctx)
        },
        manager
    )
}
</code></pre>



</details>

<a name="0x2_coin_manager_add_additional_metadata"></a>

## Function `add_additional_metadata`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_add_additional_metadata">add_additional_metadata</a>&lt;T, Value: store&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, value: Value)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_add_additional_metadata">add_additional_metadata</a>&lt;T, Value: store&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    value: Value
) {
    <b>assert</b>!(!df::exists_(&manager.id, b"additional_metadata"), <a href="coin_manager.md#0x2_coin_manager_EAdditionalMetadataAlreadyExists">EAdditionalMetadataAlreadyExists</a>);
    df::add(&<b>mut</b> manager.id, b"additional_metadata", value);
}
</code></pre>



</details>

<a name="0x2_coin_manager_replace_additional_metadata"></a>

## Function `replace_additional_metadata`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_replace_additional_metadata">replace_additional_metadata</a>&lt;T, Value: store, OldValue: store&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, value: Value): OldValue
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_replace_additional_metadata">replace_additional_metadata</a>&lt;T, Value: store, OldValue: store&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    value: Value
): OldValue {
    <b>assert</b>!(df::exists_(&manager.id, b"additional_metadata"), <a href="coin_manager.md#0x2_coin_manager_EAdditionalMetadataDoesNotExist">EAdditionalMetadataDoesNotExist</a>);
    <b>let</b> old_value = df::remove&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;, OldValue&gt;(&<b>mut</b> manager.id, b"additional_metadata");
    df::add(&<b>mut</b> manager.id, b"additional_metadata", value);
    old_value
}
</code></pre>



</details>

<a name="0x2_coin_manager_additional_metadata"></a>

## Function `additional_metadata`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_additional_metadata">additional_metadata</a>&lt;T, Value: store&gt;(manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): &Value
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_additional_metadata">additional_metadata</a>&lt;T, Value: store&gt;(
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;
): &Value {
    <b>assert</b>!(df::exists_(&manager.id, b"additional_metadata"), <a href="coin_manager.md#0x2_coin_manager_EAdditionalMetadataDoesNotExist">EAdditionalMetadataDoesNotExist</a>);
    <b>let</b> meta: &Value = df::borrow(&manager.id, b"additional_metadata");
    meta
}
</code></pre>



</details>

<a name="0x2_coin_manager_enforce_maximum_supply"></a>

## Function `enforce_maximum_supply`

A one-time callable function to set a maximum mintable supply on a coin.
This can only be set once and is irrevertable.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_enforce_maximum_supply">enforce_maximum_supply</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, maximum_supply: u64)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_enforce_maximum_supply">enforce_maximum_supply</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    maximum_supply: u64
) {
    <b>assert</b>!(<a href="../move-stdlib/option.md#0x1_option_is_none">option::is_none</a>(&manager.maximum_supply), <a href="coin_manager.md#0x2_coin_manager_EMaximumSupplyAlreadySet">EMaximumSupplyAlreadySet</a>);
    <a href="../move-stdlib/option.md#0x1_option_fill">option::fill</a>(&<b>mut</b> manager.maximum_supply, maximum_supply);
}
</code></pre>



</details>

<a name="0x2_coin_manager_renounce_ownership"></a>

## Function `renounce_ownership`

A irreversible action renouncing manager ownership which can be called if you hold the <code><a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a></code>.
This action provides <code>Coin</code> holders with some assurances if called, namely that there will
not be any new minting or changes to the metadata from this point onward. The maximum supply
will be set to the current supply and will not be changed any more afterwards.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_renounce_ownership">renounce_ownership</a>&lt;T&gt;(cap: <a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_renounce_ownership">renounce_ownership</a>&lt;T&gt;(
    cap: <a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;
) {
    // Deleting the Cap
    <b>let</b> <a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a> { id } = cap;
    <a href="object.md#0x2_object_delete">object::delete</a>(id);

    // Updating the maximum supply <b>to</b> the total supply
    <b>let</b> total_supply = <a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>(manager);
    <b>if</b>(manager.<a href="coin_manager.md#0x2_coin_manager_has_maximum_supply">has_maximum_supply</a>()) {
        <a href="../move-stdlib/option.md#0x1_option_swap">option::swap</a>(&<b>mut</b> manager.maximum_supply, total_supply);
    } <b>else</b> {
        <a href="../move-stdlib/option.md#0x1_option_fill">option::fill</a>(&<b>mut</b> manager.maximum_supply, total_supply);
    };

    // Setting ownership renounced <b>to</b> <b>true</b>
    manager.ownership_renounced = <b>true</b>;

    <a href="event.md#0x2_event_emit">event::emit</a>(<a href="coin_manager.md#0x2_coin_manager_OwnershipRenounced">OwnershipRenounced</a> {
        coin_name: <a href="../move-stdlib/type_name.md#0x1_type_name_into_string">type_name::into_string</a>(<a href="../move-stdlib/type_name.md#0x1_type_name_get">type_name::get</a>&lt;T&gt;())
    });
}
</code></pre>



</details>

<a name="0x2_coin_manager_is_immutable"></a>

## Function `is_immutable`

Convenience function allowing users to query if the ownership of this <code>Coin</code>
and thus the ability to mint new <code>Coin</code> has been renounced.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_is_immutable">is_immutable</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_is_immutable">is_immutable</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): bool {
    manager.ownership_renounced
}
</code></pre>



</details>

<a name="0x2_coin_manager_metadata"></a>

## Function `metadata`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_metadata">metadata</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): &<a href="coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_metadata">metadata</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): &CoinMetadata&lt;T&gt; {
    &manager.metadata
}
</code></pre>



</details>

<a name="0x2_coin_manager_total_supply"></a>

## Function `total_supply`

Get the total supply as a number


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): u64 {
    <a href="coin.md#0x2_coin_total_supply">coin::total_supply</a>(&manager.treasury_cap)
}
</code></pre>



</details>

<a name="0x2_coin_manager_maximum_supply"></a>

## Function `maximum_supply`

Get the maximum supply possible as a number.
If no maximum set it's the maximum u64 possible


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_maximum_supply">maximum_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_maximum_supply">maximum_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): u64 {
    <a href="../move-stdlib/option.md#0x1_option_get_with_default">option::get_with_default</a>(&manager.maximum_supply, 18_446_744_073_709_551_615u64)
}
</code></pre>



</details>

<a name="0x2_coin_manager_available_supply"></a>

## Function `available_supply`

Convenience function returning the remaining supply that can be minted still


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_available_supply">available_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_available_supply">available_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): u64 {
    <a href="coin_manager.md#0x2_coin_manager_maximum_supply">maximum_supply</a>(manager) - <a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>(manager)
}
</code></pre>



</details>

<a name="0x2_coin_manager_has_maximum_supply"></a>

## Function `has_maximum_supply`

Returns if a maximum supply has been set for this Coin or not


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_has_maximum_supply">has_maximum_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_has_maximum_supply">has_maximum_supply</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): bool {
    <a href="../move-stdlib/option.md#0x1_option_is_some">option::is_some</a>(&manager.maximum_supply)
}
</code></pre>



</details>

<a name="0x2_coin_manager_supply_immut"></a>

## Function `supply_immut`

Get immutable reference to the treasury's <code>Supply</code>.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_supply_immut">supply_immut</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): &<a href="balance.md#0x2_balance_Supply">balance::Supply</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_supply_immut">supply_immut</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): &Supply&lt;T&gt; {
    <a href="coin.md#0x2_coin_supply_immut">coin::supply_immut</a>(&manager.treasury_cap)
}
</code></pre>



</details>

<a name="0x2_coin_manager_mint"></a>

## Function `mint`

Create a coin worth <code>value</code> and increase the total supply
in <code>cap</code> accordingly.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_mint">mint</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, value: u64, ctx: &<b>mut</b> <a href="tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="coin.md#0x2_coin_Coin">coin::Coin</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_mint">mint</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    value: u64,
    ctx: &<b>mut</b> TxContext
): Coin&lt;T&gt; {
    <b>assert</b>!(<a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>(manager) + value &lt;= <a href="coin_manager.md#0x2_coin_manager_maximum_supply">maximum_supply</a>(manager), <a href="coin_manager.md#0x2_coin_manager_EMaximumSupplyReached">EMaximumSupplyReached</a>);
    <a href="coin.md#0x2_coin_mint">coin::mint</a>(&<b>mut</b> manager.treasury_cap, value, ctx)
}
</code></pre>



</details>

<a name="0x2_coin_manager_mint_balance"></a>

## Function `mint_balance`

Mint some amount of T as a <code>Balance</code> and increase the total
supply in <code>cap</code> accordingly.
Aborts if <code>value</code> + <code>cap.total_supply</code> >= U64_MAX


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_mint_balance">mint_balance</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, value: u64): <a href="balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_mint_balance">mint_balance</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    value: u64
): Balance&lt;T&gt; {
    <b>assert</b>!(<a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>(manager) + value &lt;= <a href="coin_manager.md#0x2_coin_manager_maximum_supply">maximum_supply</a>(manager), <a href="coin_manager.md#0x2_coin_manager_EMaximumSupplyReached">EMaximumSupplyReached</a>);
    <a href="coin.md#0x2_coin_mint_balance">coin::mint_balance</a>(&<b>mut</b> manager.treasury_cap, value)
}
</code></pre>



</details>

<a name="0x2_coin_manager_burn"></a>

## Function `burn`

Destroy the coin <code>c</code> and decrease the total supply in <code>cap</code>
accordingly.


<pre><code><b>public</b> entry <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_burn">burn</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, c: <a href="coin.md#0x2_coin_Coin">coin::Coin</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_burn">burn</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    c: Coin&lt;T&gt;
): u64 {
    <a href="coin.md#0x2_coin_burn">coin::burn</a>(&<b>mut</b> manager.treasury_cap, c)
}
</code></pre>



</details>

<a name="0x2_coin_manager_mint_and_transfer"></a>

## Function `mint_and_transfer`

Mint <code>amount</code> of <code>Coin</code> and send it to <code>recipient</code>. Invokes <code><a href="coin_manager.md#0x2_coin_manager_mint">mint</a>()</code>.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_mint_and_transfer">mint_and_transfer</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, amount: u64, recipient: <b>address</b>, ctx: &<b>mut</b> <a href="tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_mint_and_transfer">mint_and_transfer</a>&lt;T&gt;(
   _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
   manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
   amount: u64,
   recipient: <b>address</b>,
   ctx: &<b>mut</b> TxContext
) {
    <b>assert</b>!(<a href="coin_manager.md#0x2_coin_manager_total_supply">total_supply</a>(manager) + amount &lt;= <a href="coin_manager.md#0x2_coin_manager_maximum_supply">maximum_supply</a>(manager), <a href="coin_manager.md#0x2_coin_manager_EMaximumSupplyReached">EMaximumSupplyReached</a>);
    <a href="coin.md#0x2_coin_mint_and_transfer">coin::mint_and_transfer</a>(&<b>mut</b> manager.treasury_cap, amount, recipient, ctx)
}
</code></pre>



</details>

<a name="0x2_coin_manager_update_name"></a>

## Function `update_name`

Update the <code>name</code> of the coin in the <code>CoinMetadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_name">update_name</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, name: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_name">update_name</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    name: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
) {
    <a href="coin.md#0x2_coin_update_name">coin::update_name</a>(&manager.treasury_cap, &<b>mut</b> manager.metadata, name)
}
</code></pre>



</details>

<a name="0x2_coin_manager_update_symbol"></a>

## Function `update_symbol`

Update the <code>symbol</code> of the coin in the <code>CoinMetadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_symbol">update_symbol</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, symbol: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_symbol">update_symbol</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    symbol: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>
) {
    <a href="coin.md#0x2_coin_update_symbol">coin::update_symbol</a>(&manager.treasury_cap, &<b>mut</b> manager.metadata, symbol)
}
</code></pre>



</details>

<a name="0x2_coin_manager_update_description"></a>

## Function `update_description`

Update the <code>description</code> of the coin in the <code>CoinMetadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_description">update_description</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, description: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_description">update_description</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    description: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
) {
    <a href="coin.md#0x2_coin_update_description">coin::update_description</a>(&manager.treasury_cap, &<b>mut</b> manager.metadata, description)
}
</code></pre>



</details>

<a name="0x2_coin_manager_update_icon_url"></a>

## Function `update_icon_url`

Update the <code><a href="url.md#0x2_url">url</a></code> of the coin in the <code>CoinMetadata</code>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_icon_url">update_icon_url</a>&lt;T&gt;(_: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">coin_manager::CoinManagerCap</a>&lt;T&gt;, manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;, <a href="url.md#0x2_url">url</a>: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_update_icon_url">update_icon_url</a>&lt;T&gt;(
    _: &<a href="coin_manager.md#0x2_coin_manager_CoinManagerCap">CoinManagerCap</a>&lt;T&gt;,
    manager: &<b>mut</b> <a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;,
    <a href="url.md#0x2_url">url</a>: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>
) {
    <a href="coin.md#0x2_coin_update_icon_url">coin::update_icon_url</a>(&manager.treasury_cap, &<b>mut</b> manager.metadata, <a href="url.md#0x2_url">url</a>)
}
</code></pre>



</details>

<a name="0x2_coin_manager_decimals"></a>

## Function `decimals`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_decimals">decimals</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): u8
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_decimals">decimals</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): u8 {
    <a href="coin.md#0x2_coin_get_decimals">coin::get_decimals</a>(&manager.metadata)
}
</code></pre>



</details>

<a name="0x2_coin_manager_name"></a>

## Function `name`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_name">name</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_name">name</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/string.md#0x1_string_String">string::String</a> {
    <a href="coin.md#0x2_coin_get_name">coin::get_name</a>(&manager.metadata)
}
</code></pre>



</details>

<a name="0x2_coin_manager_symbol"></a>

## Function `symbol`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_symbol">symbol</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_symbol">symbol</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a> {
    <a href="coin.md#0x2_coin_get_symbol">coin::get_symbol</a>(&manager.metadata)
}
</code></pre>



</details>

<a name="0x2_coin_manager_description"></a>

## Function `description`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_description">description</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_description">description</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/string.md#0x1_string_String">string::String</a> {
    <a href="coin.md#0x2_coin_get_description">coin::get_description</a>(&manager.metadata)
}
</code></pre>



</details>

<a name="0x2_coin_manager_icon_url"></a>

## Function `icon_url`



<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_icon_url">icon_url</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">coin_manager::CoinManager</a>&lt;T&gt;): <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="url.md#0x2_url_Url">url::Url</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="coin_manager.md#0x2_coin_manager_icon_url">icon_url</a>&lt;T&gt;(manager: &<a href="coin_manager.md#0x2_coin_manager_CoinManager">CoinManager</a>&lt;T&gt;): Option&lt;Url&gt; {
    <a href="coin.md#0x2_coin_get_icon_url">coin::get_icon_url</a>(&manager.metadata)
}
</code></pre>



</details>
