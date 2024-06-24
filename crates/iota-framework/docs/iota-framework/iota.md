---
title: Module `0x2::iota`
---

Coin<IOTA> is the token used to pay for gas in IOTA.
It has 9 decimals, and the smallest unit (10^-9) is called "nano".


-  [Struct `IOTA`](#0x2_iota_IOTA)
-  [Constants](#@Constants_0)
-  [Function `new`](#0x2_iota_new)
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



<a name="0x2_iota_NANO_PER_IOTA"></a>

The amount of Nanos per IOTA token based on the fact that nano is
10^-9 of a IOTA token


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_NANO_PER_IOTA">NANO_PER_IOTA</a>: u64 = 1000000000;
</code></pre>



<a name="0x2_iota_TOTAL_SUPPLY_NANO"></a>

The total supply of IOTA denominated in Nano (4.6 Billion * 10^9)


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_TOTAL_SUPPLY_NANO">TOTAL_SUPPLY_NANO</a>: u64 = 4600000000000000000;
</code></pre>



<a name="0x2_iota_new"></a>

## Function `new`

Register the <code><a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a></code> Coin to acquire its <code>Supply</code>.
This should be called only once during genesis creation.


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_new">new</a>(ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_new">new</a>(ctx: &<b>mut</b> TxContext): Balance&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt; {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);
    <b>assert</b>!(ctx.epoch() == 0, <a href="../iota-framework/iota.md#0x2_iota_EAlreadyMinted">EAlreadyMinted</a>);

    <b>let</b> (treasury, metadata) = <a href="../iota-framework/coin.md#0x2_coin_create_currency">coin::create_currency</a>(
        <a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a> {},
        9,
        b"<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>",
        b"<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>",
        b"The main (gas)token of the <a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a> Network.",
        <a href="../move-stdlib/option.md#0x1_option_some">option::some</a>(<a href="../iota-framework/url.md#0x2_url_new_unsafe_from_bytes">url::new_unsafe_from_bytes</a>(b"https://<a href="../iota-framework/iota.md#0x2_iota">iota</a>.org/logo.png")),
        ctx
    );
    <a href="../iota-framework/transfer.md#0x2_transfer_public_freeze_object">transfer::public_freeze_object</a>(metadata);
    <b>let</b> <b>mut</b> supply = treasury.treasury_into_supply();
    <b>let</b> total_iota = supply.increase_supply(<a href="../iota-framework/iota.md#0x2_iota_TOTAL_SUPPLY_NANO">TOTAL_SUPPLY_NANO</a>);
    supply.destroy_supply();
    total_iota
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
