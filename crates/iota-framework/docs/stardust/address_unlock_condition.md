---
title: Module `0x107a::address_unlock_condition`
---



-  [Function `unlock_alias_address_owned_basic`](#0x107a_address_unlock_condition_unlock_alias_address_owned_basic)
-  [Function `unlock_alias_address_owned_nft`](#0x107a_address_unlock_condition_unlock_alias_address_owned_nft)
-  [Function `unlock_alias_address_owned_alias`](#0x107a_address_unlock_condition_unlock_alias_address_owned_alias)
-  [Function `unlock_alias_address_owned_treasury`](#0x107a_address_unlock_condition_unlock_alias_address_owned_treasury)
-  [Function `unlock_nft_address_owned_basic`](#0x107a_address_unlock_condition_unlock_nft_address_owned_basic)
-  [Function `unlock_nft_address_owned_nft`](#0x107a_address_unlock_condition_unlock_nft_address_owned_nft)
-  [Function `unlock_nft_address_owned_alias`](#0x107a_address_unlock_condition_unlock_nft_address_owned_alias)


<pre><code><b>use</b> <a href="alias.md#0x107a_alias">0x107a::alias</a>;
<b>use</b> <a href="alias_output.md#0x107a_alias_output">0x107a::alias_output</a>;
<b>use</b> <a href="basic_output.md#0x107a_basic_output">0x107a::basic_output</a>;
<b>use</b> <a href="nft.md#0x107a_nft">0x107a::nft</a>;
<b>use</b> <a href="nft_output.md#0x107a_nft_output">0x107a::nft_output</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
</code></pre>



<a name="0x107a_address_unlock_condition_unlock_alias_address_owned_basic"></a>

## Function `unlock_alias_address_owned_basic`

Unlock a <code>BasicOutput</code> locked to the alias address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_basic">unlock_alias_address_owned_basic</a>(self: &<b>mut</b> <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>, output_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>&gt;): <a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_basic">unlock_alias_address_owned_basic</a>(
  self: &<b>mut</b> Alias,
  output_to_unlock: Receiving&lt;BasicOutput&gt;
): BasicOutput {
    <a href="basic_output.md#0x107a_basic_output_receive">basic_output::receive</a>(self.id(), output_to_unlock)
}
</code></pre>



</details>

<a name="0x107a_address_unlock_condition_unlock_alias_address_owned_nft"></a>

## Function `unlock_alias_address_owned_nft`

Unlock an <code>NftOutput</code> locked to the alias address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_nft">unlock_alias_address_owned_nft</a>(self: &<b>mut</b> <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>, output_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&gt;): <a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_nft">unlock_alias_address_owned_nft</a>(
  self: &<b>mut</b> Alias,
  output_to_unlock: Receiving&lt;NftOutput&gt;,
): NftOutput {
    <a href="nft_output.md#0x107a_nft_output_receive">nft_output::receive</a>(self.id(), output_to_unlock)
}
</code></pre>



</details>

<a name="0x107a_address_unlock_condition_unlock_alias_address_owned_alias"></a>

## Function `unlock_alias_address_owned_alias`

Unlock an <code>AliasOutput</code> locked to the alias address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_alias">unlock_alias_address_owned_alias</a>(self: &<b>mut</b> <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>, output_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&gt;): <a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_alias">unlock_alias_address_owned_alias</a>(
  self: &<b>mut</b> Alias,
  output_to_unlock: Receiving&lt;AliasOutput&gt;,
): AliasOutput {
    <a href="alias_output.md#0x107a_alias_output_receive">alias_output::receive</a>(self.id(), output_to_unlock)
}
</code></pre>



</details>

<a name="0x107a_address_unlock_condition_unlock_alias_address_owned_treasury"></a>

## Function `unlock_alias_address_owned_treasury`

Unlock a <code>TreasuryCap</code> locked to the alias address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_treasury">unlock_alias_address_owned_treasury</a>&lt;T: store, key&gt;(self: &<b>mut</b> <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>, treasury_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;T&gt;&gt;): <a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_alias_address_owned_treasury">unlock_alias_address_owned_treasury</a>&lt;T: key + store&gt;(
  self: &<b>mut</b> Alias,
  treasury_to_unlock: Receiving&lt;TreasuryCap&lt;T&gt;&gt;,
): TreasuryCap&lt;T&gt; {
    <a href="../iota-framework/transfer.md#0x2_transfer_public_receive">transfer::public_receive</a>(self.id(), treasury_to_unlock)
}
</code></pre>



</details>

<a name="0x107a_address_unlock_condition_unlock_nft_address_owned_basic"></a>

## Function `unlock_nft_address_owned_basic`

Unlock a <code>BasicOutput</code> locked to the <code>Nft</code> address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_nft_address_owned_basic">unlock_nft_address_owned_basic</a>(self: &<b>mut</b> <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>, output_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>&gt;): <a href="basic_output.md#0x107a_basic_output_BasicOutput">basic_output::BasicOutput</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_nft_address_owned_basic">unlock_nft_address_owned_basic</a>(
  self: &<b>mut</b> Nft,
  output_to_unlock: Receiving&lt;BasicOutput&gt;,
): BasicOutput {
    <a href="basic_output.md#0x107a_basic_output_receive">basic_output::receive</a>(self.id(), output_to_unlock)
}
</code></pre>



</details>

<a name="0x107a_address_unlock_condition_unlock_nft_address_owned_nft"></a>

## Function `unlock_nft_address_owned_nft`

Unlock an <code>NftOutput</code> locked to the <code>Nft</code> address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_nft_address_owned_nft">unlock_nft_address_owned_nft</a>(self: &<b>mut</b> <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>, output_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>&gt;): <a href="nft_output.md#0x107a_nft_output_NftOutput">nft_output::NftOutput</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_nft_address_owned_nft">unlock_nft_address_owned_nft</a>(
  self: &<b>mut</b> Nft,
  output_to_unlock: Receiving&lt;NftOutput&gt;,
): NftOutput {
    <a href="nft_output.md#0x107a_nft_output_receive">nft_output::receive</a>(self.id(), output_to_unlock)
}
</code></pre>



</details>

<a name="0x107a_address_unlock_condition_unlock_nft_address_owned_alias"></a>

## Function `unlock_nft_address_owned_alias`

Unlock an <code>AliasOutput</code> locked to the <code>Nft</code> address.


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_nft_address_owned_alias">unlock_nft_address_owned_alias</a>(self: &<b>mut</b> <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>, output_to_unlock: <a href="../iota-framework/transfer.md#0x2_transfer_Receiving">transfer::Receiving</a>&lt;<a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>&gt;): <a href="alias_output.md#0x107a_alias_output_AliasOutput">alias_output::AliasOutput</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="address_unlock_condition.md#0x107a_address_unlock_condition_unlock_nft_address_owned_alias">unlock_nft_address_owned_alias</a>(
  self: &<b>mut</b> Nft,
  output_to_unlock: Receiving&lt;AliasOutput&gt;,
): AliasOutput {
    <a href="alias_output.md#0x107a_alias_output_receive">alias_output::receive</a>(self.id(), output_to_unlock)
}
</code></pre>



</details>
