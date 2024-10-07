---
title: Module `0x2::smr`
---

Coin<SMR> is the token used for migrated Shimmer users.
It has 6 decimals, and the smallest unit (10^-6) is called "glow".


-  [Struct `SMR`](#0x2_smr_SMR)
-  [Constants](#@Constants_0)
-  [Function `init`](#0x2_smr_init)
-  [Function `transfer`](#0x2_smr_transfer)


<pre><code><b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-framework/url.md#0x2_url">0x2::url</a>;
</code></pre>



<a name="0x2_smr_SMR"></a>

## Struct `SMR`

Name of the coin


<pre><code><b>struct</b> <a href="../iota-framework/smr.md#0x2_smr_SMR">SMR</a> <b>has</b> drop
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>dummy_field: bool</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x2_smr_ENotSystemAddress"></a>

Sender is not @0x0 the system address.


<pre><code><b>const</b> <a href="../iota-framework/smr.md#0x2_smr_ENotSystemAddress">ENotSystemAddress</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1;
</code></pre>



<a name="0x2_smr_EAlreadyMinted"></a>



<pre><code><b>const</b> <a href="../iota-framework/smr.md#0x2_smr_EAlreadyMinted">EAlreadyMinted</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x2_smr_GLOW_PER_SMR"></a>

The amount of Glows per Shimmer token based on the fact that micros is
10^-6 of a Shimmer token


<pre><code><b>const</b> <a href="../iota-framework/smr.md#0x2_smr_GLOW_PER_SMR">GLOW_PER_SMR</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1000000;
</code></pre>



<a name="0x2_smr_TOTAL_SUPPLY_GLOWS"></a>

The total supply of Shimmer denominated in Glows


<pre><code><b>const</b> <a href="../iota-framework/smr.md#0x2_smr_TOTAL_SUPPLY_GLOWS">TOTAL_SUPPLY_GLOWS</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1813620509061365;
</code></pre>



<a name="0x2_smr_init"></a>

## Function `init`

Register the <code>SHIMMER</code> coin.


<pre><code><b>fun</b> <a href="../iota-framework/smr.md#0x2_smr_init">init</a>(witness: <a href="../iota-framework/smr.md#0x2_smr_SMR">smr::SMR</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/smr.md#0x2_smr_init">init</a>(witness: <a href="../iota-framework/smr.md#0x2_smr_SMR">SMR</a>, ctx: &<b>mut</b> TxContext) {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/smr.md#0x2_smr_ENotSystemAddress">ENotSystemAddress</a>);
    <b>assert</b>!(ctx.epoch() == 0, <a href="../iota-framework/smr.md#0x2_smr_EAlreadyMinted">EAlreadyMinted</a>);

    <b>let</b> (treasury, metadata) = <a href="../iota-framework/coin.md#0x2_coin_create_currency">coin::create_currency</a>(
            witness,
            6,
            b"Shimmer",
            b"<a href="../iota-framework/smr.md#0x2_smr_SMR">SMR</a>",
            b"The original Shimmer (<a href="../iota-framework/smr.md#0x2_smr_SMR">SMR</a>) token <b>as</b> inherited from the Shimmer Network.",
            <a href="../move-stdlib/option.md#0x1_option_some">option::some</a>(<a href="../iota-framework/url.md#0x2_url_new_unsafe_from_bytes">url::new_unsafe_from_bytes</a>(b"https://<a href="../iota-framework/iota.md#0x2_iota">iota</a>.org/<a href="../iota-framework/smr.md#0x2_smr">smr</a>-logo.png")),
            ctx
        );
    <a href="../iota-framework/transfer.md#0x2_transfer_public_freeze_object">transfer::public_freeze_object</a>(metadata);
    <b>let</b> supply = treasury.treasury_into_supply();
    supply.destroy_supply();
}
</code></pre>



</details>

<a name="0x2_smr_transfer"></a>

## Function `transfer`



<pre><code><b>public</b> entry <b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(c: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/smr.md#0x2_smr_SMR">smr::SMR</a>&gt;, recipient: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(c: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/smr.md#0x2_smr_SMR">SMR</a>&gt;, recipient: <b>address</b>) {
    <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(c, recipient)
}
</code></pre>



</details>
