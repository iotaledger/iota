---
title: Module `0x10cf::labeler`
---



-  [Resource `LabelerCap`](#0x10cf_labeler_LabelerCap)
-  [Constants](#@Constants_0)
-  [Function `create_labeler_cap`](#0x10cf_labeler_create_labeler_cap)
-  [Function `destroy_labeler_cap`](#0x10cf_labeler_destroy_labeler_cap)
-  [Function `type_name`](#0x10cf_labeler_type_name)


<pre><code><b>use</b> <a href="../move-stdlib/ascii.md#0x1_ascii">0x1::ascii</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../move-stdlib/type_name.md#0x1_type_name">0x1::type_name</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
<b>use</b> <a href="../iota-framework/types.md#0x2_types">0x2::types</a>;
</code></pre>



<a name="0x10cf_labeler_LabelerCap"></a>

## Resource `LabelerCap`

<code><a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a></code> allows to create labels of the specific type <code>L</code>.
Can be publicly transferred like any other object.


<pre><code><b>struct</b> <a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; <b>has</b> store, key
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


<a name="0x10cf_labeler_ENotOneTimeWitness"></a>

Error code for when a type passed to the <code>create_labeler_cap</code> function is not a one-time witness.


<pre><code><b>const</b> <a href="labeler.md#0x10cf_labeler_ENotOneTimeWitness">ENotOneTimeWitness</a>: u64 = 0;
</code></pre>



<a name="0x10cf_labeler_create_labeler_cap"></a>

## Function `create_labeler_cap`

Create a <code><a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a></code> instance.
Can be created only by consuming a one time witness.


<pre><code><b>public</b> <b>fun</b> <a href="labeler.md#0x10cf_labeler_create_labeler_cap">create_labeler_cap</a>&lt;L: drop&gt;(witness: L, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="labeler.md#0x10cf_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="labeler.md#0x10cf_labeler_create_labeler_cap">create_labeler_cap</a>&lt;L: drop&gt;(witness: L, ctx: &<b>mut</b> TxContext): <a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; {
    <b>assert</b>!(<a href="../iota-framework/types.md#0x2_types_is_one_time_witness">types::is_one_time_witness</a>(&witness), <a href="labeler.md#0x10cf_labeler_ENotOneTimeWitness">ENotOneTimeWitness</a>);

    <a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
    }
}
</code></pre>



</details>

<a name="0x10cf_labeler_destroy_labeler_cap"></a>

## Function `destroy_labeler_cap`

Delete a <code><a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a></code> instance.
If a capability is destroyed, it is impossible to add the related labels.


<pre><code><b>public</b> <b>fun</b> <a href="labeler.md#0x10cf_labeler_destroy_labeler_cap">destroy_labeler_cap</a>&lt;L&gt;(cap: <a href="labeler.md#0x10cf_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="labeler.md#0x10cf_labeler_destroy_labeler_cap">destroy_labeler_cap</a>&lt;L&gt;(cap: <a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a>&lt;L&gt;) {
    <b>let</b> <a href="labeler.md#0x10cf_labeler_LabelerCap">LabelerCap</a>&lt;L&gt; {
        id,
    } = cap;

    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);
}
</code></pre>



</details>

<a name="0x10cf_labeler_type_name"></a>

## Function `type_name`

Return a fully qualified type name with the original package IDs
that is used as type related a label value.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="../move-stdlib/type_name.md#0x1_type_name">type_name</a>&lt;L&gt;(): <a href="../move-stdlib/string.md#0x1_string_String">string::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="../move-stdlib/type_name.md#0x1_type_name">type_name</a>&lt;L&gt;(): String {
    <a href="../move-stdlib/string.md#0x1_string_from_ascii">string::from_ascii</a>(std::type_name::get_with_original_ids&lt;L&gt;().into_string())
}
</code></pre>



</details>
