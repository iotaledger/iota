---
title: Module `0x2::labeler`
---



-  [Resource `LabelerCap`](#0x2_labeler_LabelerCap)
-  [Constants](#@Constants_0)
-  [Function `create_labeler_cap`](#0x2_labeler_create_labeler_cap)
-  [Function `destroy_labeler_cap`](#0x2_labeler_destroy_labeler_cap)


<pre><code><b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-framework/types.md#0x2_types">0x2::types</a>;
</code></pre>



<a name="0x2_labeler_LabelerCap"></a>

## Resource `LabelerCap`

<code><a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a></code> allows to create labels of the specific type <code>L</code>.
Can be publicly transferred like any other object.


<pre><code><b>struct</b> <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; <b>has</b> store, key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>

</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x2_labeler_ENotOneTimeWitness"></a>

Error code for when a type passed to the <code>create_labeler_cap</code> function is not a one-time witness.


<pre><code><b>const</b> <a href="../iota-framework/labeler.md#0x2_labeler_ENotOneTimeWitness">ENotOneTimeWitness</a>: <a href="../move-stdlib/u64.md#0x1_u64">u64</a> = 0;
</code></pre>



<a name="0x2_labeler_create_labeler_cap"></a>

## Function `create_labeler_cap`

Create a <code><a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a></code> instance.
Can be created only by consuming a one time witness.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/labeler.md#0x2_labeler_create_labeler_cap">create_labeler_cap</a>&lt;L: drop&gt;(witness: L, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/labeler.md#0x2_labeler_create_labeler_cap">create_labeler_cap</a>&lt;L: drop&gt;(witness: L, ctx: &<b>mut</b> TxContext): <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; {
    <b>assert</b>!(iota::types::is_one_time_witness(&witness), <a href="../iota-framework/labeler.md#0x2_labeler_ENotOneTimeWitness">ENotOneTimeWitness</a>);

    <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
    }
}
</code></pre>



</details>

<a name="0x2_labeler_destroy_labeler_cap"></a>

## Function `destroy_labeler_cap`

Delete a <code><a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a></code> instance.
If a capability is destroyed, it is impossible to add the related labels.


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/labeler.md#0x2_labeler_destroy_labeler_cap">destroy_labeler_cap</a>&lt;L&gt;(cap: <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../iota-framework/labeler.md#0x2_labeler_destroy_labeler_cap">destroy_labeler_cap</a>&lt;L&gt;(cap: <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a>&lt;L&gt;) {
    <b>let</b> <a href="../iota-framework/labeler.md#0x2_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; {
        id,
    } = cap;

    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);
}
</code></pre>



</details>
