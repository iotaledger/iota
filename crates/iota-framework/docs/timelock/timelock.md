---
title: Module `0x10cf::timelock`
---

A timelock implementation.


-  [Resource `TimeLock`](#0x10cf_timelock_TimeLock)
-  [Constants](#@Constants_0)
-  [Function `lock`](#0x10cf_timelock_lock)
-  [Function `lock_to`](#0x10cf_timelock_lock_to)
-  [Function `lock_with_label`](#0x10cf_timelock_lock_with_label)
-  [Function `lock_with_label_to`](#0x10cf_timelock_lock_with_label_to)
-  [Function `unlock`](#0x10cf_timelock_unlock)
-  [Function `expiration_timestamp_ms`](#0x10cf_timelock_expiration_timestamp_ms)
-  [Function `is_locked`](#0x10cf_timelock_is_locked)
-  [Function `remaining_time`](#0x10cf_timelock_remaining_time)
-  [Function `locked`](#0x10cf_timelock_locked)
-  [Function `locked_mut`](#0x10cf_timelock_locked_mut)
-  [Function `label`](#0x10cf_timelock_label)
-  [Function `is_labeled_with`](#0x10cf_timelock_is_labeled_with)
-  [Function `pack`](#0x10cf_timelock_pack)
-  [Function `unpack`](#0x10cf_timelock_unpack)
-  [Function `self_transfer`](#0x10cf_timelock_self_transfer)
-  [Function `check_expiration_timestamp_ms`](#0x10cf_timelock_check_expiration_timestamp_ms)


<pre><code><b>use</b> <a href="labeler.md#0x10cf_labeler">0x10cf::labeler</a>;
<b>use</b> <a href="../move-stdlib/option.md#0x1_option">0x1::option</a>;
<b>use</b> <a href="../move-stdlib/string.md#0x1_string">0x1::string</a>;
<b>use</b> <a href="../iota-framework/object.md#0x2_object">0x2::object</a>;
<b>use</b> <a href="../iota-framework/transfer.md#0x2_transfer">0x2::transfer</a>;
<b>use</b> <a href="../iota-framework/tx_context.md#0x2_tx_context">0x2::tx_context</a>;
</code></pre>



<a name="0x10cf_timelock_TimeLock"></a>

## Resource `TimeLock`

<code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code> struct that holds a locked object.


<pre><code><b>struct</b> <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T: store&gt; <b>has</b> key
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>id: <a href="../iota-framework/object.md#0x2_object_UID">object::UID</a></code>
</dt>
<dd>

</dd>
<dt>
<code>locked: T</code>
</dt>
<dd>
 The locked object.
</dd>
<dt>
<code>expiration_timestamp_ms: u64</code>
</dt>
<dd>
 This is the epoch time stamp of when the lock expires.
</dd>
<dt>
<code>label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;</code>
</dt>
<dd>
 Timelock related label.
</dd>
</dl>


</details>

<a name="@Constants_0"></a>

## Constants


<a name="0x10cf_timelock_EExpireEpochIsPast"></a>

Error code for when the expire timestamp of the lock is in the past.


<pre><code><b>const</b> <a href="timelock.md#0x10cf_timelock_EExpireEpochIsPast">EExpireEpochIsPast</a>: u64 = 0;
</code></pre>



<a name="0x10cf_timelock_ENotExpiredYet"></a>

Error code for when the lock has not expired yet.


<pre><code><b>const</b> <a href="timelock.md#0x10cf_timelock_ENotExpiredYet">ENotExpiredYet</a>: u64 = 1;
</code></pre>



<a name="0x10cf_timelock_lock"></a>

## Function `lock`

Function to lock an object till a unix timestamp in milliseconds.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock">lock</a>&lt;T: store&gt;(locked: T, expiration_timestamp_ms: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock">lock</a>&lt;T: store&gt;(locked: T, expiration_timestamp_ms: u64, ctx: &<b>mut</b> TxContext): <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt; {
    // Check that `expiration_timestamp_ms` is valid.
    <a href="timelock.md#0x10cf_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms, ctx);

    // Create a <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <a href="timelock.md#0x10cf_timelock_pack">pack</a>(locked, expiration_timestamp_ms, <a href="../move-stdlib/option.md#0x1_option_none">option::none</a>(), ctx)
}
</code></pre>



</details>

<a name="0x10cf_timelock_lock_to"></a>

## Function `lock_to`

Function to lock an object <code>obj</code> until <code>expiration_timestamp_ms</code> and transfer it to address <code><b>to</b></code>.
Since <code>Timelock&lt;T&gt;</code> does not support public transfer, use this function to lock an object to an address.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock_to">lock_to</a>&lt;T: store&gt;(obj: T, <b>to</b>: <b>address</b>, expiration_timestamp_ms: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock_to">lock_to</a>&lt;T: store&gt;(
    obj: T,
    <b>to</b>: <b>address</b>,
    expiration_timestamp_ms: u64,
    ctx: &<b>mut</b> TxContext
) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(<a href="timelock.md#0x10cf_timelock_lock">lock</a>(obj, expiration_timestamp_ms, ctx), <b>to</b>);
}
</code></pre>



</details>

<a name="0x10cf_timelock_lock_with_label"></a>

## Function `lock_with_label`

Function to lock a labeled object till a unix timestamp in milliseconds.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock_with_label">lock_with_label</a>&lt;T: store, L&gt;(_: &<a href="labeler.md#0x10cf_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;, locked: T, expiration_timestamp_ms: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock_with_label">lock_with_label</a>&lt;T: store, L&gt;(
    _: &LabelerCap&lt;L&gt;,
    locked: T,
    expiration_timestamp_ms: u64,
    ctx: &<b>mut</b> TxContext
): <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt; {
    // Check that `expiration_timestamp_ms` is valid.
    <a href="timelock.md#0x10cf_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms, ctx);

    // Calculate a label value.
    <b>let</b> label = <a href="labeler.md#0x10cf_labeler_type_name">labeler::type_name</a>&lt;L&gt;();

    // Create a labeled <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <a href="timelock.md#0x10cf_timelock_pack">pack</a>(locked, expiration_timestamp_ms, <a href="../move-stdlib/option.md#0x1_option_some">option::some</a>(label), ctx)
}
</code></pre>



</details>

<a name="0x10cf_timelock_lock_with_label_to"></a>

## Function `lock_with_label_to`

Function to lock a labeled object <code>obj</code> until <code>expiration_timestamp_ms</code> and transfer it to address <code><b>to</b></code>.
Since <code>Timelock&lt;T&gt;</code> does not support public transfer, use this function to lock a labeled object to an address.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock_with_label_to">lock_with_label_to</a>&lt;T: store, L&gt;(<a href="labeler.md#0x10cf_labeler">labeler</a>: &<a href="labeler.md#0x10cf_labeler_LabelerCap">labeler::LabelerCap</a>&lt;L&gt;, obj: T, <b>to</b>: <b>address</b>, expiration_timestamp_ms: u64, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_lock_with_label_to">lock_with_label_to</a>&lt;T: store, L&gt;(
    <a href="labeler.md#0x10cf_labeler">labeler</a>: &LabelerCap&lt;L&gt;,
    obj: T,
    <b>to</b>: <b>address</b>,
    expiration_timestamp_ms: u64,
    ctx: &<b>mut</b> TxContext
) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(<a href="timelock.md#0x10cf_timelock_lock_with_label">lock_with_label</a>(<a href="labeler.md#0x10cf_labeler">labeler</a>, obj, expiration_timestamp_ms, ctx), <b>to</b>);
}
</code></pre>



</details>

<a name="0x10cf_timelock_unlock"></a>

## Function `unlock`

Function to unlock the object from a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_unlock">unlock</a>&lt;T: store&gt;(self: <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): T
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_unlock">unlock</a>&lt;T: store&gt;(self: <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext): T {
    // Unpack the <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <b>let</b> (locked, expiration_timestamp_ms, _) = <a href="timelock.md#0x10cf_timelock_unpack">unpack</a>(self);

    // Check <b>if</b> the lock <b>has</b> expired.
    <b>assert</b>!(<a href="timelock.md#0x10cf_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a> &lt;= ctx.epoch_timestamp_ms(), <a href="timelock.md#0x10cf_timelock_ENotExpiredYet">ENotExpiredYet</a>);

    locked
}
</code></pre>



</details>

<a name="0x10cf_timelock_expiration_timestamp_ms"></a>

## Function `expiration_timestamp_ms`

Function to get the expiration timestamp of a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;): u64 {
    self.expiration_timestamp_ms
}
</code></pre>



</details>

<a name="0x10cf_timelock_is_locked"></a>

## Function `is_locked`

Function to check if a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code> is locked.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_is_locked">is_locked</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_is_locked">is_locked</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext): bool {
    self.<a href="timelock.md#0x10cf_timelock_remaining_time">remaining_time</a>(ctx) &gt; 0
}
</code></pre>



</details>

<a name="0x10cf_timelock_remaining_time"></a>

## Function `remaining_time`

Function to get the remaining time of a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.
Returns 0 if the lock has expired.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_remaining_time">remaining_time</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): u64
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_remaining_time">remaining_time</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext): u64 {
    // Get the epoch timestamp.
    <b>let</b> current_timestamp_ms = ctx.epoch_timestamp_ms();

    // Check <b>if</b> the lock <b>has</b> expired.
    <b>if</b> (self.<a href="timelock.md#0x10cf_timelock_expiration_timestamp_ms">expiration_timestamp_ms</a> &lt; current_timestamp_ms) {
        <b>return</b> 0
    };

    // Calculate the remaining time.
    self.expiration_timestamp_ms - current_timestamp_ms
}
</code></pre>



</details>

<a name="0x10cf_timelock_locked"></a>

## Function `locked`

Function to get the locked object of a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_locked">locked</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): &T
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_locked">locked</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;): &T {
    &self.locked
}
</code></pre>



</details>

<a name="0x10cf_timelock_locked_mut"></a>

## Function `locked_mut`

Function to get a mutable reference to the locked object of a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.
Must not be callable from the outside, as one could modify the locked object.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelock.md#0x10cf_timelock_locked_mut">locked_mut</a>&lt;T: store&gt;(self: &<b>mut</b> <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): &<b>mut</b> T
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelock.md#0x10cf_timelock_locked_mut">locked_mut</a>&lt;T: store&gt;(self: &<b>mut</b> <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;): &<b>mut</b> T {
    &<b>mut</b> self.locked
}
</code></pre>



</details>

<a name="0x10cf_timelock_label"></a>

## Function `label`

Function to get the label of a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_label">label</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_label">label</a>&lt;T: store&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;): Option&lt;String&gt; {
    self.label
}
</code></pre>



</details>

<a name="0x10cf_timelock_is_labeled_with"></a>

## Function `is_labeled_with`

Check if a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code> is labeled with the type <code>L</code>.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_is_labeled_with">is_labeled_with</a>&lt;T: store, L&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_is_labeled_with">is_labeled_with</a>&lt;T: store, L&gt;(self: &<a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;): bool {
    <b>if</b> (self.label.is_some()) {
        self.label.borrow() == <a href="labeler.md#0x10cf_labeler_type_name">labeler::type_name</a>&lt;L&gt;()
    }
    <b>else</b> {
        <b>false</b>
    }
}
</code></pre>



</details>

<a name="0x10cf_timelock_pack"></a>

## Function `pack`

A utility function to pack a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelock.md#0x10cf_timelock_pack">pack</a>&lt;T: store&gt;(locked: T, expiration_timestamp_ms: u64, label: <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;, ctx: &<b>mut</b> <a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>): <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelock.md#0x10cf_timelock_pack">pack</a>&lt;T: store&gt;(
    locked: T,
    expiration_timestamp_ms: u64,
    label: Option&lt;String&gt;,
    ctx: &<b>mut</b> TxContext): <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;
{
    // Create a <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a> {
        id: <a href="../iota-framework/object.md#0x2_object_new">object::new</a>(ctx),
        locked,
        expiration_timestamp_ms,
        label,
    }
}
</code></pre>



</details>

<a name="0x10cf_timelock_unpack"></a>

## Function `unpack`

An utility function to unpack a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code>.


<pre><code><b>public</b>(<b>friend</b>) <b>fun</b> <a href="timelock.md#0x10cf_timelock_unpack">unpack</a>&lt;T: store&gt;(lock: <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;): (T, u64, <a href="../move-stdlib/option.md#0x1_option_Option">option::Option</a>&lt;<a href="../move-stdlib/string.md#0x1_string_String">string::String</a>&gt;)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b>(package) <b>fun</b> <a href="timelock.md#0x10cf_timelock_unpack">unpack</a>&lt;T: store&gt;(lock: <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;): (T, u64, Option&lt;String&gt;) {
    // Unpack the <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <b>let</b> <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a> {
        id,
        locked,
        expiration_timestamp_ms,
        label,
    } = lock;

    // Delete the <a href="timelock.md#0x10cf_timelock">timelock</a>.
    <a href="../iota-framework/object.md#0x2_object_delete">object::delete</a>(id);

    (locked, expiration_timestamp_ms, label)
}
</code></pre>



</details>

<a name="0x10cf_timelock_self_transfer"></a>

## Function `self_transfer`

A utility function to transfer a <code><a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a></code> to its original owner.


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_self_transfer">self_transfer</a>&lt;T: store&gt;(lock: <a href="timelock.md#0x10cf_timelock_TimeLock">timelock::TimeLock</a>&lt;T&gt;, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="timelock.md#0x10cf_timelock_self_transfer">self_transfer</a>&lt;T: store&gt;(lock: <a href="timelock.md#0x10cf_timelock_TimeLock">TimeLock</a>&lt;T&gt;, ctx: &TxContext) {
    <a href="../iota-framework/transfer.md#0x2_transfer_transfer">transfer::transfer</a>(lock, ctx.sender())
}
</code></pre>



</details>

<a name="0x10cf_timelock_check_expiration_timestamp_ms"></a>

## Function `check_expiration_timestamp_ms`

An utility function to check that the <code>expiration_timestamp_ms</code> value is valid.


<pre><code><b>fun</b> <a href="timelock.md#0x10cf_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms: u64, ctx: &<a href="../iota-framework/tx_context.md#0x2_tx_context_TxContext">tx_context::TxContext</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="timelock.md#0x10cf_timelock_check_expiration_timestamp_ms">check_expiration_timestamp_ms</a>(expiration_timestamp_ms: u64, ctx: &TxContext) {
    // Get the epoch timestamp.
    <b>let</b> epoch_timestamp_ms = ctx.epoch_timestamp_ms();

    // Check that `expiration_timestamp_ms` is valid.
    <b>assert</b>!(expiration_timestamp_ms &gt; epoch_timestamp_ms, <a href="timelock.md#0x10cf_timelock_EExpireEpochIsPast">EExpireEpochIsPast</a>);
}
</code></pre>



</details>
