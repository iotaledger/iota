---
title: Module `0x107a::capped_coin`
---



-  [Resource `MaxSupplyPolicy`](#0x107a_capped_coin_MaxSupplyPolicy)
-  [Constants](#@Constants_0)
-  [Function `create_max_supply_policy`](#0x107a_capped_coin_create_max_supply_policy)
-  [Function `total_supply`](#0x107a_capped_coin_total_supply)
-  [Function `supply_immut`](#0x107a_capped_coin_supply_immut)
-  [Function `mint`](#0x107a_capped_coin_mint)
-  [Function `mint_balance`](#0x107a_capped_coin_mint_balance)
-  [Function `burn`](#0x107a_capped_coin_burn)
-  [Function `mint_and_transfer`](#0x107a_capped_coin_mint_and_transfer)
-  [Function `update_name`](#0x107a_capped_coin_update_name)
-  [Function `update_symbol`](#0x107a_capped_coin_update_symbol)
-  [Function `update_description`](#0x107a_capped_coin_update_description)
-  [Function `update_icon_url`](#0x107a_capped_coin_update_icon_url)


<pre><code><b>use</b> <a href="../move-stdlib/ascii.md#0x1_ascii">0x1::ascii</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x107a_capped_coin_MaxSupplyPolicy"></a>

## Resource `MaxSupplyPolicy`

The policy wrapper that ensures the supply of a <code>Coin</code> never exceeds the maximum supply.


<pre><code><b>struct</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt; <b>has</b> store, key
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
<code>maximum_supply: u64</code>
</dt>
<dd>
 The maximum supply.
</dd>
<dt>
<code>treasury_cap: <a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;T&gt;</code>
</dt>
<dd>
 The wrapped <code>TreasuryCap</code>.
</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x107a_capped_coin_EMaximumSupplyReached"></a>

The error returned when the maximum supply reached.


<pre><code><b>const</b> <a href="capped_coin.md#0x107a_capped_coin_EMaximumSupplyReached">EMaximumSupplyReached</a>: u64 = 0;
</code></pre>



<a name="0x107a_capped_coin_create_max_supply_policy"></a>

## Function `create_max_supply_policy`

Wrap a <code>TreasuryCap</code> into a <code><a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a></code> to prevent minting of tokens > max supply.
Be careful, once you add a maximum supply you will not be able to change it or get rid of it!
This gives coin holders a guarantee that the maximum supply of that specific coin will never change.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_create_max_supply_policy">create_max_supply_policy</a>&lt;T&gt;(treasury_cap: <a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;T&gt;, maximum_supply: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_create_max_supply_policy">create_max_supply_policy</a>&lt;T&gt;(
    treasury_cap: TreasuryCap&lt;T&gt;,
    maximum_supply: u64,
    ctx: &<b>mut</b> TxContext
): <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt; {
    <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        maximum_supply,
        treasury_cap
    }
}
</code></pre>



</details>

<a name="0x107a_capped_coin_total_supply"></a>

## Function `total_supply`

Return the total number of <code>T</code>'s in circulation.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_total_supply">total_supply</a>&lt;T&gt;(policy: &<a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_total_supply">total_supply</a>&lt;T&gt;(policy: &<a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;): u64 {
    <a href="../iota-framework/coin.md#0x2_coin_total_supply">coin::total_supply</a>(&policy.treasury_cap)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_supply_immut"></a>

## Function `supply_immut`

Get immutable reference to the treasury's <code>Supply</code>.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_supply_immut">supply_immut</a>&lt;T&gt;(policy: &<a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;): &<a href="../iota-framework/balance.md#0x2_balance_Supply">balance::Supply</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_supply_immut">supply_immut</a>&lt;T&gt;(policy: &<a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;): &Supply&lt;T&gt; {
    <a href="../iota-framework/coin.md#0x2_coin_supply_immut">coin::supply_immut</a>(&policy.treasury_cap)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_mint"></a>

## Function `mint`

Create a <code>Coin</code> worth <code>value</code> and increase the total supply in <code>cap</code> accordingly.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_mint">mint</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, value: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_mint">mint</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    value: u64,
    ctx: &<b>mut</b> TxContext
): Coin&lt;T&gt; {
    <b>assert</b>!(<a href="capped_coin.md#0x107a_capped_coin_total_supply">total_supply</a>(policy) + value &lt;= policy.maximum_supply, <a href="capped_coin.md#0x107a_capped_coin_EMaximumSupplyReached">EMaximumSupplyReached</a>);
    <a href="../iota-framework/coin.md#0x2_coin_mint">coin::mint</a>(&<b>mut</b> policy.treasury_cap, value, ctx)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_mint_balance"></a>

## Function `mint_balance`

Mint some amount of <code>T</code> as a <code>Balance</code> and increase the total supply in <code>cap</code> accordingly.
Aborts if <code>value</code> + <code>cap.total_supply</code> >= <code>U64_MAX</code>.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_mint_balance">mint_balance</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, value: u64): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_mint_balance">mint_balance</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    value: u64
): Balance&lt;T&gt; {
    <b>assert</b>!(<a href="capped_coin.md#0x107a_capped_coin_total_supply">total_supply</a>(policy) + value &lt;= policy.maximum_supply, <a href="capped_coin.md#0x107a_capped_coin_EMaximumSupplyReached">EMaximumSupplyReached</a>);
    <a href="../iota-framework/coin.md#0x2_coin_mint_balance">coin::mint_balance</a>(&<b>mut</b> policy.treasury_cap, value)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_burn"></a>

## Function `burn`

Destroy the coin <code>c</code> and decrease the total supply in <code>cap</code> accordingly.


<pre><code><b>public</b> entry <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_burn">burn</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, c: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_burn">burn</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;, c: Coin&lt;T&gt;): u64 {
    <a href="../iota-framework/coin.md#0x2_coin_burn">coin::burn</a>(&<b>mut</b> policy.treasury_cap, c)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_mint_and_transfer"></a>

## Function `mint_and_transfer`

Mint <code>amount</code> of <code>Coin</code> and send it to the <code>recipient</code>. Invokes <code><a href="capped_coin.md#0x107a_capped_coin_mint">mint</a>()</code>.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_mint_and_transfer">mint_and_transfer</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, amount: u64, recipient: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_mint_and_transfer">mint_and_transfer</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    amount: u64,
    recipient: <b>address</b>,
    ctx: &<b>mut</b> TxContext
) {
    <b>assert</b>!(<a href="capped_coin.md#0x107a_capped_coin_total_supply">total_supply</a>(policy) + amount &lt;= policy.maximum_supply, <a href="capped_coin.md#0x107a_capped_coin_EMaximumSupplyReached">EMaximumSupplyReached</a>);
    <a href="../iota-framework/coin.md#0x2_coin_mint_and_transfer">coin::mint_and_transfer</a>(&<b>mut</b> policy.treasury_cap, amount, recipient, ctx)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_update_name"></a>

## Function `update_name`

Update the <code>name</code> of the coin in the <code>CoinMetadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_name">update_name</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, metadata: &<b>mut</b> <a href="../iota-framework/coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;, name: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_name">update_name</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    metadata: &<b>mut</b> CoinMetadata&lt;T&gt;,
    name: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
) {
    <a href="../iota-framework/coin.md#0x2_coin_update_name">coin::update_name</a>(&policy.treasury_cap, metadata, name)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_update_symbol"></a>

## Function `update_symbol`

Update the <code>symbol</code> of the coin in the <code>CoinMetadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_symbol">update_symbol</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, metadata: &<b>mut</b> <a href="../iota-framework/coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;, symbol: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_symbol">update_symbol</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    metadata: &<b>mut</b> CoinMetadata&lt;T&gt;,
    symbol: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>
) {
    <a href="../iota-framework/coin.md#0x2_coin_update_symbol">coin::update_symbol</a>(&policy.treasury_cap, metadata, symbol)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_update_description"></a>

## Function `update_description`

Update the <code>description</code> of the coin in the <code>CoinMetadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_description">update_description</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, metadata: &<b>mut</b> <a href="../iota-framework/coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;, description: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_description">update_description</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    metadata: &<b>mut</b> CoinMetadata&lt;T&gt;,
    description: <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
) {
    <a href="../iota-framework/coin.md#0x2_coin_update_description">coin::update_description</a>(&policy.treasury_cap, metadata, description)
}
</code></pre>



</details>

<a name="0x107a_capped_coin_update_icon_url"></a>

## Function `update_icon_url`

Update the <code><a href="../iota-framework/url.md#0x2_url">url</a></code> of the coin in the <code>CoinMetadata</code>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_icon_url">update_icon_url</a>&lt;T&gt;(policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">capped_coin::MaxSupplyPolicy</a>&lt;T&gt;, metadata: &<b>mut</b> <a href="../iota-framework/coin.md#0x2_coin_CoinMetadata">coin::CoinMetadata</a>&lt;T&gt;, <a href="../iota-framework/url.md#0x2_url">url</a>: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="capped_coin.md#0x107a_capped_coin_update_icon_url">update_icon_url</a>&lt;T&gt;(
    policy: &<b>mut</b> <a href="capped_coin.md#0x107a_capped_coin_MaxSupplyPolicy">MaxSupplyPolicy</a>&lt;T&gt;,
    metadata: &<b>mut</b> CoinMetadata&lt;T&gt;,
    <a href="../iota-framework/url.md#0x2_url">url</a>: <a href="../move-stdlib/ascii.md#0x1_ascii_String">ascii::String</a>
) {
    <a href="../iota-framework/coin.md#0x2_coin_update_icon_url">coin::update_icon_url</a>(&policy.treasury_cap, metadata, <a href="../iota-framework/url.md#0x2_url">url</a>)
}
</code></pre>



</details>
