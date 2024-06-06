---
title: Module `0x107a::alias`
---



-  [Resource `Alias`](#0x107a_alias_Alias)
-  [Function `destroy`](#0x107a_alias_destroy)
-  [Function `legacy_state_controller`](#0x107a_alias_legacy_state_controller)
-  [Function `state_index`](#0x107a_alias_state_index)
-  [Function `state_metadata`](#0x107a_alias_state_metadata)
-  [Function `sender`](#0x107a_alias_sender)
-  [Function `metadata`](#0x107a_alias_metadata)
-  [Function `immutable_issuer`](#0x107a_alias_immutable_issuer)
-  [Function `immutable_metadata`](#0x107a_alias_immutable_metadata)
-  [Function `id`](#0x107a_alias_id)


<pre><code><b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
</code></pre>



<a name="0x107a_alias_Alias"></a>

## Resource `Alias`

The persisted Alias object from Stardust, without tokens and assets.
Outputs owned the AliasID/Address in Stardust will be sent to this object and
have to be received via this object once extracted from <code>AliasOutput</code>.


<pre><code><b>struct</b> <a href="alias.md#0x107a_alias_Alias">Alias</a> <b>has</b> store, key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>
 The ID of the Alias = hash of the Output ID that created the Alias Output in Stardust.
 This is the AliasID from Stardust.
</dd>
<dt>
<code>legacy_state_controller: <b>address</b></code>
</dt>
<dd>
 The last State Controller address assigned before the migration.
</dd>
<dt>
<code>state_index: u32</code>
</dt>
<dd>
 A counter increased by 1 every time the alias was state transitioned.
</dd>
<dt>
<code>state_metadata: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 State metadata that can be used to store additional information.
</dd>
<dt>
<code>sender: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;</code>
</dt>
<dd>
 The sender feature.
</dd>
<dt>
<code>metadata: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 The metadata feature.
</dd>
<dt>
<code>immutable_issuer: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;</code>
</dt>
<dd>
 The immutable issuer feature.
</dd>
<dt>
<code>immutable_metadata: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;</code>
</dt>
<dd>
 The immutable metadata feature.
</dd>
</dl>


</details>

<a name="0x107a_alias_destroy"></a>

## Function `destroy`

Destroy the <code><a href="alias.md#0x107a_alias_Alias">Alias</a></code> object, equivalent to <code>burning</code> an Alias Output in Stardust.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_destroy">destroy</a>(self: <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_destroy">destroy</a>(self: <a href="alias.md#0x107a_alias_Alias">Alias</a>) {
    <b>let</b> <a href="alias.md#0x107a_alias_Alias">Alias</a> {
        id,
        legacy_state_controller: _,
        state_index: _,
        state_metadata: _,
        sender: _,
        metadata: _,
        immutable_issuer: _,
        immutable_metadata: _,
    } = self;

    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);
}
</code></pre>



</details>

<a name="0x107a_alias_legacy_state_controller"></a>

## Function `legacy_state_controller`

Get the Alias's <code>legacy_state_controller</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_legacy_state_controller">legacy_state_controller</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<b>address</b>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_legacy_state_controller">legacy_state_controller</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): &<b>address</b> {
    &self.legacy_state_controller
}
</code></pre>



</details>

<a name="0x107a_alias_state_index"></a>

## Function `state_index`

Get the Alias's <code>state_index</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_state_index">state_index</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): u32
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_state_index">state_index</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): u32 {
    self.state_index
}
</code></pre>



</details>

<a name="0x107a_alias_state_metadata"></a>

## Function `state_metadata`

Get the Alias's <code>state_metadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_state_metadata">state_metadata</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_state_metadata">state_metadata</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): &Option&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt; {
    &self.state_metadata
}
</code></pre>



</details>

<a name="0x107a_alias_sender"></a>

## Function `sender`

Get the Alias's <code>sender</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_sender">sender</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_sender">sender</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): &Option&lt;<b>address</b>&gt; {
    &self.sender
}
</code></pre>



</details>

<a name="0x107a_alias_metadata"></a>

## Function `metadata`

Get the Alias's <code>metadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_metadata">metadata</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_metadata">metadata</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): &Option&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt; {
    &self.metadata
}
</code></pre>



</details>

<a name="0x107a_alias_immutable_issuer"></a>

## Function `immutable_issuer`

Get the Alias's <code>immutable_sender</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_immutable_issuer">immutable_issuer</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<b>address</b>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_immutable_issuer">immutable_issuer</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): &Option&lt;<b>address</b>&gt; {
    &self.immutable_issuer
}
</code></pre>



</details>

<a name="0x107a_alias_immutable_metadata"></a>

## Function `immutable_metadata`

Get the Alias's <code>immutable_metadata</code>.


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_immutable_metadata">immutable_metadata</a>(self: &<a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="alias.md#0x107a_alias_immutable_metadata">immutable_metadata</a>(self: &<a href="alias.md#0x107a_alias_Alias">Alias</a>): &Option&lt;<a href="../move-stdlib/vector.md#0x1_vector">vector</a>&lt;u8&gt;&gt; {
    &self.immutable_metadata
}
</code></pre>



</details>

<a name="0x107a_alias_id"></a>

## Function `id`

Get the Alias's id.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="alias.md#0x107a_alias_id">id</a>(self: &<b>mut</b> <a href="alias.md#0x107a_alias_Alias">alias::Alias</a>): &<b>mut</b> <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(<a href="../iota-framework/package.md#0x2_package">package</a>) <b>fun</b> <a href="alias.md#0x107a_alias_id">id</a>(self: &<b>mut</b> <a href="alias.md#0x107a_alias_Alias">Alias</a>): &<b>mut</b> UID {
    &<b>mut</b> self.id
}
</code></pre>



</details>
