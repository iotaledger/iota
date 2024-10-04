---
title: Module `0x2::iota`
---

Coin<IOTA> is the token used to pay for gas in IOTA.
It has 9 decimals, and the smallest unit (10^-9) is called "nano".


-  [Struct `IOTA`](#0x2_iota_IOTA)
-  [Struct `IotaTreasuryCap`](#0x2_iota_IotaTreasuryCap)
-  [Constants](#@Constants_0)
-  [Function `new`](#0x2_iota_new)
-  [Function `transfer`](#0x2_iota_transfer)
-  [Function `mint`](#0x2_iota_mint)
-  [Function `mint_balance`](#0x2_iota_mint_balance)
-  [Function `burn`](#0x2_iota_burn)
-  [Function `burn_balance`](#0x2_iota_burn_balance)
-  [Function `total_supply`](#0x2_iota_total_supply)


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

<a name="0x2_iota_IotaTreasuryCap"></a>

## Struct `IotaTreasuryCap`

The IOTA token treasury capability.
Protects the token from unauthorized changes.


<pre><code><b>struct</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a> <b>has</b> store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>inner: <a href="../iota-framework/coin.md#0x2_coin_TreasuryCap">coin::TreasuryCap</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;</code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x2_iota_ENotSystemAddress"></a>

Sender is not @0x0 the system address.


<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 1;
</code></pre>



<a name="0x2_iota_EAlreadyMinted"></a>



<pre><code><b>const</b> <a href="../iota-framework/iota.md#0x2_iota_EAlreadyMinted">EAlreadyMinted</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x2_iota_new"></a>

## Function `new`

Register the <code><a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a></code> Coin to acquire <code><a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a></code>.
This should be called only once during genesis creation.


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_new">new</a>(ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_new">new</a>(ctx: &<b>mut</b> TxContext): <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a> {
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

    <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a> {
        inner: treasury,
    }
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

<a name="0x2_iota_mint"></a>

## Function `mint`

Create an IOTA coin worth <code>value</code> and increase the total supply in <code>cap</code> accordingly.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_mint">mint</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_mint">mint</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a>, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<b>mut</b> TxContext): Coin&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt; {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);

    cap.inner.<a href="../iota-framework/iota.md#0x2_iota_mint">mint</a>(value, ctx)
}
</code></pre>



</details>

<a name="0x2_iota_mint_balance"></a>

## Function `mint_balance`

Mint some amount of IOTA as a <code>Balance</code> and increase the total supply in <code>cap</code> accordingly.
Aborts if <code>value</code> + <code>cap.inner.total_supply</code> >= U64_MAX


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_mint_balance">mint_balance</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_mint_balance">mint_balance</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a>, value: <a href="../move-stdlib/u64.md#0x1_u64">u64</a>, ctx: &TxContext): Balance&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt; {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);

    cap.inner.<a href="../iota-framework/iota.md#0x2_iota_mint_balance">mint_balance</a>(value)
}
</code></pre>



</details>

<a name="0x2_iota_burn"></a>

## Function `burn`

Destroy the IOTA coin <code>c</code> and decrease the total supply in <code>cap</code> accordingly.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_burn">burn</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>, c: <a href="../iota-framework/coin.md#0x2_coin_Coin">coin::Coin</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_burn">burn</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a>, c: Coin&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt;, ctx: &TxContext): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);

    cap.inner.<a href="../iota-framework/iota.md#0x2_iota_burn">burn</a>(c)
}
</code></pre>



</details>

<a name="0x2_iota_burn_balance"></a>

## Function `burn_balance`

Destroy the IOTA balance <code>b</code> and decrease the total supply in <code>cap</code> accordingly.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_burn_balance">burn_balance</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>, b: <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">iota::IOTA</a>&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_burn_balance">burn_balance</a>(cap: &<b>mut</b> <a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a>, b: Balance&lt;<a href="../iota-framework/iota.md#0x2_iota_IOTA">IOTA</a>&gt;, ctx: &TxContext): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    <b>assert</b>!(ctx.sender() == @0x0, <a href="../iota-framework/iota.md#0x2_iota_ENotSystemAddress">ENotSystemAddress</a>);

    cap.inner.supply_mut().decrease_supply(b)
}
</code></pre>



</details>

<a name="0x2_iota_total_supply"></a>

## Function `total_supply`

Return the total number of IOTA's in circulation.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_total_supply">total_supply</a>(cap: &<a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">iota::IotaTreasuryCap</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/iota.md#0x2_iota_total_supply">total_supply</a>(cap: &<a href="../iota-framework/iota.md#0x2_iota_IotaTreasuryCap">IotaTreasuryCap</a>): <a href="../move-stdlib/u64.md#0x1_u64">u64</a> {
    cap.inner.<a href="../iota-framework/iota.md#0x2_iota_total_supply">total_supply</a>()
}
</code></pre>



</details>
