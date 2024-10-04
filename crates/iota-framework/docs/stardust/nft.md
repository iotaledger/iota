---
title: Module `0x107a::nft`
---



-  [Struct `NFT`](#0x107a_nft_NFT)
-  [Resource `Nft`](#0x107a_nft_Nft)
-  [Function `init`](#0x107a_nft_init)
-  [Function `destroy`](#0x107a_nft_destroy)
-  [Function `legacy_sender`](#0x107a_nft_legacy_sender)
-  [Function `metadata`](#0x107a_nft_metadata)
-  [Function `tag`](#0x107a_nft_tag)
-  [Function `immutable_issuer`](#0x107a_nft_immutable_issuer)
-  [Function `immutable_metadata`](#0x107a_nft_immutable_metadata)
-  [Function `id`](#0x107a_nft_id)


<pre><code><b>use</b> <a href="irc27.md#0x107a_irc27">0x107a::irc27</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../iota-framework/display.md#0x2_display">0x2::display</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/package.md#0x2_package">0x2::package</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x107a_nft_NFT"></a>

## Struct `NFT`

One Time Witness.


<pre><code><b>struct</b> <a href="nft.md#0x107a_nft_NFT">NFT</a> <b>has</b> drop
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

<a name="0x107a_nft_Nft"></a>

## Resource `Nft`

The Stardust NFT representation.


<pre><code><b>struct</b> <a href="nft.md#0x107a_nft_Nft">Nft</a> <b>has</b> store, key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>
 The Nft's ID is nested from Stardust.
</dd>
<dt>
<code>legacy_sender: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;</code>
</dt>
<dd>
 The sender feature holds the last sender address assigned before the migration and
 is not supported by the protocol after it.
</dd>
<dt>
<code>metadata: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 The metadata feature.
</dd>
<dt>
<code>tag: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 The tag feature.
</dd>
<dt>
<code>immutable_issuer: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;</code>
</dt>
<dd>
 The immutable issuer feature.
</dd>
<dt>
<code>immutable_metadata: <a href="irc27.md#0x107a_irc27_Irc27Metadata">irc27::Irc27Metadata</a></code>
</dt>
<dd>
 The immutable metadata feature.
</dd>
</dl>


</details>

<a name="0x107a_nft_init"></a>

## Function `init`

The <code><a href="nft.md#0x107a_nft_Nft">Nft</a></code> module initializer.


<pre><code><b>fun</b> <a href="nft.md#0x107a_nft_init">init</a>(otw: <a href="nft.md#0x107a_nft_NFT">nft::NFT</a>, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="nft.md#0x107a_nft_init">init</a>(otw: <a href="nft.md#0x107a_nft_NFT">NFT</a>, ctx: &<b>mut</b> TxContext) {
    // Claim the <b>module</b> publisher.
    <b>let</b> publisher = <a href="../iota-framework/package.md#0x2_package_claim">package::claim</a>(otw, ctx);

    // Build a `Display` <a href="../iota-framework/object.md#0x2_object">object</a>.
    <b>let</b> keys = <a href="../move-stdlib/vector.md#0x1_vector">vector</a>[
        // The Iota standard fields.
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"name"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"image_url"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"description"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"creator"),

        // The extra IRC27-nested fields.
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"version"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"media_type"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"collection_name"),

        // The issuer of the <a href="nft.md#0x107a_nft_NFT">NFT</a>. Equivalent <b>to</b> IRC-27 `collectionId`.
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"immutable_issuer"),
    ];

    <b>let</b> values = <a href="../move-stdlib/vector.md#0x1_vector">vector</a>[
        // The Iota standard fields.
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.name}"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.uri}"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.description}"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.issuer_name}"),

        // The extra IRC27-nested fields.
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.version}"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.media_type}"),
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_metadata.collection_name}"),

        // The issuer of the <a href="nft.md#0x107a_nft_NFT">NFT</a>. Equivalent <b>to</b> IRC-27 `collectionId`.
        <a href="../move-stdlib/string.md#0x1_string_utf8">string::utf8</a>(b"{immutable_issuer}"),
    ];

    <b>let</b> <b>mut</b> <a href="../iota-framework/display.md#0x2_display">display</a> = <a href="../iota-framework/display.md#0x2_display_new_with_fields">display::new_with_fields</a>&lt;<a href="nft.md#0x107a_nft_Nft">Nft</a>&gt;(
        &publisher,
        keys,
        values,
        ctx
    );

    // Commit the first version of `Display` <b>to</b> <b>apply</b> changes.
    <a href="../iota-framework/display.md#0x2_display">display</a>.update_version();

    // Burn the publisher <a href="../iota-framework/object.md#0x2_object">object</a>.
    <a href="../iota-framework/package.md#0x2_package_burn_publisher">package::burn_publisher</a>(publisher);

    // Freeze the <a href="../iota-framework/display.md#0x2_display">display</a> <a href="../iota-framework/object.md#0x2_object">object</a>.
    iota::transfer::public_freeze_object(<a href="../iota-framework/display.md#0x2_display">display</a>);
}
</code></pre>



</details>

<a name="0x107a_nft_destroy"></a>

## Function `destroy`

Permanently destroy an <code><a href="nft.md#0x107a_nft_Nft">Nft</a></code> object.


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_destroy">destroy</a>(<a href="nft.md#0x107a_nft">nft</a>: <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_destroy">destroy</a>(<a href="nft.md#0x107a_nft">nft</a>: <a href="nft.md#0x107a_nft_Nft">Nft</a>) {
    <b>let</b> <a href="nft.md#0x107a_nft_Nft">Nft</a> {
        id,
        legacy_sender: _,
        metadata: _,
        tag: _,
        immutable_issuer: _,
        immutable_metadata,
    } = <a href="nft.md#0x107a_nft">nft</a>;

    <a href="irc27.md#0x107a_irc27_destroy">irc27::destroy</a>(immutable_metadata);

    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);
}
</code></pre>



</details>

<a name="0x107a_nft_legacy_sender"></a>

## Function `legacy_sender`

Get the Nft's <code>legacy_sender</code>.


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_legacy_sender">legacy_sender</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">nft::Nft</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_legacy_sender">legacy_sender</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">Nft</a>): &Option&lt;<b>address</b>&gt; {
    &<a href="nft.md#0x107a_nft">nft</a>.legacy_sender
}
</code></pre>



</details>

<a name="0x107a_nft_metadata"></a>

## Function `metadata`

Get the Nft's <code>metadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_metadata">metadata</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">nft::Nft</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_metadata">metadata</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">Nft</a>): &Option&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt; {
    &<a href="nft.md#0x107a_nft">nft</a>.metadata
}
</code></pre>



</details>

<a name="0x107a_nft_tag"></a>

## Function `tag`

Get the Nft's <code>tag</code>.


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_tag">tag</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">nft::Nft</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_tag">tag</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">Nft</a>): &Option&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt; {
    &<a href="nft.md#0x107a_nft">nft</a>.tag
}
</code></pre>



</details>

<a name="0x107a_nft_immutable_issuer"></a>

## Function `immutable_issuer`

Get the Nft's <code>immutable_sender</code>.


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_immutable_issuer">immutable_issuer</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">nft::Nft</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_immutable_issuer">immutable_issuer</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">Nft</a>): &Option&lt;<b>address</b>&gt; {
    &<a href="nft.md#0x107a_nft">nft</a>.immutable_issuer
}
</code></pre>



</details>

<a name="0x107a_nft_immutable_metadata"></a>

## Function `immutable_metadata`

Get the Nft's <code>immutable_metadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_immutable_metadata">immutable_metadata</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">nft::Nft</a>): &<a href="irc27.md#0x107a_irc27_Irc27Metadata">irc27::Irc27Metadata</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="nft.md#0x107a_nft_immutable_metadata">immutable_metadata</a>(<a href="nft.md#0x107a_nft">nft</a>: &<a href="nft.md#0x107a_nft_Nft">Nft</a>): &Irc27Metadata {
    &<a href="nft.md#0x107a_nft">nft</a>.immutable_metadata
}
</code></pre>



</details>

<a name="0x107a_nft_id"></a>

## Function `id`

Get the Nft's id.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="nft.md#0x107a_nft_id">id</a>(self: &<b>mut</b> <a href="nft.md#0x107a_nft_Nft">nft::Nft</a>): &<b>mut</b> <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<a href="../iota-framework/package.md#0x2_package">package</a>) <b>fun</b> <a href="nft.md#0x107a_nft_id">id</a>(self: &<b>mut</b> <a href="nft.md#0x107a_nft_Nft">Nft</a>): &<b>mut</b> UID {
    &<b>mut</b> self.id
}
</code></pre>



</details>
