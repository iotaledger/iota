---
title: Module `0x2::iota`
---

Coin<IOTA> is the token used to pay for gas in Iota.
It has 9 decimals, and the smallest unit (10^-9) is called "micros".


-  [Struct `IOTA`](#0x2_iota_IOTA)
-  [Constants](#@Constants_0)
-  [Function `new`](#0x2_iota_new)
-  [Function `increase_supply`](#0x2_iota_increase_supply)
-  [Function `transfer`](#0x2_iota_transfer)


<pre><code><b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-framework/url.md#0x2_url">0x2::url</a>;
</code></pre>



<a name="0x2_iota_IOTA"></a>

## Struct `IOTA`

Name of the coin


<pre><code><b>struct</b> <a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a> <b>has</b> drop
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


<a name="0x2_iota_ENotSystemAddress"></a>

Sender is not @0x0 the system address.


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>: u64 = 1;
</code></pre>



<a name="0x2_iota_EAlreadyMinted"></a>



<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_EAlreadyMinted">EAlreadyMinted</a>: u64 = 0;
</code></pre>



<a name="0x2_iota_MICROS_PER_IOTA"></a>

The amount of Micros per Iota token based on the fact that micros is
10^-9 of a Iota token


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_MICROS_PER_IOTA">MICROS_PER_IOTA</a>: u64 = 1000000000;
</code></pre>



<a name="0x2_iota_TOTAL_SUPPLY_IOTA"></a>

The total supply of Iota denominated in whole Iota tokens (10 Billion)


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_TOTAL_SUPPLY_IOTA">TOTAL_SUPPLY_IOTA</a>: u64 = 10000000000;
</code></pre>



<a name="0x2_iota_TOTAL_SUPPLY_MICROS"></a>

The total supply of Iota denominated in Micros (10 Billion * 10^9)


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_TOTAL_SUPPLY_MICROS">TOTAL_SUPPLY_MICROS</a>: u64 = 10000000000000000000;
</code></pre>



<a name="0x2_iota_new"></a>

## Function `new`

Register the <code><a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a></code> Coin to acquire its <code>TreasuryCap</code>.
This should be called only once during genesis creation.


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_new">new</a>(ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_new">new</a>(ctx: &<b>mut</b> TxContext): TreasuryCap&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt; {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);
    <b>assert</b>!(ctx.epoch() == 0, <a href="../iota-framework/iota.md#0x2_iota_EAlreadyMinted">EAlreadyMinted</a>);

    <b>let</b> (treasury, metadata) = <a href="../iota-framework/coin.md#0x2_coin_create_currency">coin::create_currency</a>(
        <a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a> {},
        9,
        b"<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>",
        b"Iota",
        // TODO: add appropriate description and logo <a href="../iota-framework/url.md#0x2_url">url</a>
        b"",
        <a href="../move-stdlib/option.md#0x1_option_none">option::none</a>(),
        ctx
    );

    <a href="../iota-framework/transfer.md#0x2_transfer_public_freeze_object">transfer::public_freeze_object</a>(metadata);

    treasury
}
</code></pre>



</details>

<a name="0x2_iota_increase_supply"></a>

## Function `increase_supply`

Increase the IOTA supply.
This should be called only once during genesis creation.


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_increase_supply">increase_supply</a>(cap: &<b>mut</b> <a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_increase_supply">increase_supply</a>(cap: &<b>mut</b> TreasuryCap&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt;, ctx: &TxContext): Balance&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt; {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);
    <b>assert</b>!(ctx.epoch() == 0, <a href="../iota-framework/iota.md#0x2_iota_EAlreadyMinted">EAlreadyMinted</a>);

    cap.supply_mut().<a href="../iota-framework/iota.md#0x2_iota_increase_supply">increase_supply</a>(<a href="../iota-framework/iota.md#0x2_iota_TOTAL_SUPPLY_MICROS">TOTAL_SUPPLY_MICROS</a>)
}
</code></pre>



</details>

<a name="0x2_iota_transfer"></a>

## Function `transfer`



<pre><code><b>public</b> entry <b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(c: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, recipient: <b>address</b>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> entry <b>fun</b> <a href="../iota-framework/transfer.md#0x2_transfer">transfer</a>(c: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt;, recipient: <b>address</b>) {
    <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(c, recipient)
}
</code></pre>



</details>
