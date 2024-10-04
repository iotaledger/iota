---
title: Module `0x107a::utilities`
---



-  [Constants](#@Constants_0)
-  [Function `extract_and_send_to`](#0x107a_utilities_extract_and_send_to)
-  [Function `extract`](#0x107a_utilities_extract)
-  [Function `extract_`](#0x107a_utilities_extract_)


<pre><code><b>use</b> <a href="../move-stdlib/ascii.md#0x1_ascii">0x1::ascii</a>;
<b>use</b> <a href="../move-stdlib/type_name.md#0x1_type_name">0x1::type_name</a>;
<b>use</b> <a href="../iota-framework/bag.md#0x2_bag">0x2::bag</a>;
<b>use</b> <a href="../iota-framework/balance.md#0x2_balance">0x2::balance</a>;
<b>use</b> <a href="../iota-framework/coin.md#0x2_coin">0x2::coin</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="@Constants_0"></a>

## Constants


<a name="0x107a_utilities_EZeroNativeTokenBalance"></a>

Returned when trying to extract a <code>Balance&lt;T&gt;</code> from a <code>Bag</code> and the balance is zero.


<pre><code><b>const</b> <a href="utilities.md#0x107a_utilities_EZeroNativeTokenBalance">EZeroNativeTokenBalance</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x107a_utilities_extract_and_send_to"></a>

## Function `extract_and_send_to`

Extract a <code>Balance&lt;T&gt;</code> from a <code>Bag</code>, create a <code>Coin</code> out of it and send it to the address.
NOTE: We return the <code>Bag</code> by value so the function can be called repeatedly in a PTB.


<pre><code><b>public</b> <b>fun</b> <a href="utilities.md#0x107a_utilities_extract_and_send_to">extract_and_send_to</a>&lt;T&gt;(<a href="../iota-framework/bag.md#0x2_bag">bag</a>: <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>, <b>to</b>: <b>address</b>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="utilities.md#0x107a_utilities_extract_and_send_to">extract_and_send_to</a>&lt;T&gt;(<b>mut</b> <a href="../iota-framework/bag.md#0x2_bag">bag</a>: Bag, <b>to</b>: <b>address</b>, ctx: &<b>mut</b> TxContext): Bag {
    <b>let</b> <a href="../iota-framework/coin.md#0x2_coin">coin</a> = <a href="../iota-framework/coin.md#0x2_coin_from_balance">coin::from_balance</a>(<a href="utilities.md#0x107a_utilities_extract_">extract_</a>&lt;T&gt;(&<b>mut</b> <a href="../iota-framework/bag.md#0x2_bag">bag</a>), ctx);
    <a href="../iota-framework/transfer.md#0x2_transfer_public_transfer">transfer::public_transfer</a>(<a href="../iota-framework/coin.md#0x2_coin">coin</a>, <b>to</b>);
    <a href="../iota-framework/bag.md#0x2_bag">bag</a>
}
</code></pre>



</details>

<a name="0x107a_utilities_extract"></a>

## Function `extract`

Extract a <code>Balance&lt;T&gt;</code> from a <code>Bag</code> and return it. Caller can decide what to do with it.
NOTE: We return the <code>Bag</code> by value so the function can be called repeatedly in a PTB.


<pre><code><b>public</b> <b>fun</b> <a href="utilities.md#0x107a_utilities_extract">extract</a>&lt;T&gt;(<a href="../iota-framework/bag.md#0x2_bag">bag</a>: <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>): (<a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>, <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="utilities.md#0x107a_utilities_extract">extract</a>&lt;T&gt;(<b>mut</b> <a href="../iota-framework/bag.md#0x2_bag">bag</a>: Bag): (Bag, Balance&lt;T&gt;) {
    <b>let</b> <a href="../iota-framework/balance.md#0x2_balance">balance</a> = <a href="utilities.md#0x107a_utilities_extract_">extract_</a>&lt;T&gt;(&<b>mut</b> <a href="../iota-framework/bag.md#0x2_bag">bag</a>);
    (<a href="../iota-framework/bag.md#0x2_bag">bag</a>, <a href="../iota-framework/balance.md#0x2_balance">balance</a>)
}
</code></pre>



</details>

<a name="0x107a_utilities_extract_"></a>

## Function `extract_`

Get a <code>Balance&lt;T&gt;</code> from a <code>Bag</code>.
Aborts if the balance is zero or if there is no balance for the type <code>T</code>.


<pre><code><b>fun</b> <a href="utilities.md#0x107a_utilities_extract_">extract_</a>&lt;T&gt;(<a href="../iota-framework/bag.md#0x2_bag">bag</a>: &<b>mut</b> <a href="../iota-framework/bag.md#0x2_bag_Bag">bag::Bag</a>): <a href="../iota-framework/balance.md#0x2_balance_Balance">balance::Balance</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="utilities.md#0x107a_utilities_extract_">extract_</a>&lt;T&gt;(<a href="../iota-framework/bag.md#0x2_bag">bag</a>: &<b>mut</b> Bag): Balance&lt;T&gt; {
    <b>let</b> key = <a href="../move-stdlib/type_name.md#0x1_type_name_get">type_name::get</a>&lt;T&gt;().into_string();

    // This call aborts <b>if</b> the key doesn't exist.
    <b>let</b> <a href="../iota-framework/balance.md#0x2_balance">balance</a> : Balance&lt;T&gt; = <a href="../iota-framework/bag.md#0x2_bag">bag</a>.remove(key);

    <b>assert</b>!(<a href="../iota-framework/balance.md#0x2_balance">balance</a>.value() != 0, <a href="utilities.md#0x107a_utilities_EZeroNativeTokenBalance">EZeroNativeTokenBalance</a>);

    <a href="../iota-framework/balance.md#0x2_balance">balance</a>
}
</code></pre>



</details>
