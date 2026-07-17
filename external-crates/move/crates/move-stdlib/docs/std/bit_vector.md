
<a name="std_bit_vector"></a>

# Module `std::bit_vector`



-  [Module Functions](#@Module_Functions_0)
    -  [<span class="move-vis move-vis-public">pub</span> `new`](#std_bit_vector_new)
-  [Structs](#@Structs_1)
    -  [<span class="move-vis move-vis-struct">struct</span> `BitVector`](#std_bit_vector_BitVector)
        -  [<span class="move-vis move-vis-public">pub</span> `is_index_set`](#std_bit_vector_is_index_set)
        -  [<span class="move-vis move-vis-public">pub</span> `length`](#std_bit_vector_length)
        -  [<span class="move-vis move-vis-public">pub</span> `longest_set_sequence_starting_at`](#std_bit_vector_longest_set_sequence_starting_at)
        -  [<span class="move-vis move-vis-public">pub</span> `set`](#std_bit_vector_set)
        -  [<span class="move-vis move-vis-public">pub</span> `shift_left`](#std_bit_vector_shift_left)
        -  [<span class="move-vis move-vis-public">pub</span> `unset`](#std_bit_vector_unset)
-  [Constants](#@Constants_2)
    -  [<span class="move-vis move-vis-error">err</span> `EINDEX`](#std_bit_vector_EINDEX)
    -  [<span class="move-vis move-vis-error">err</span> `ELENGTH`](#std_bit_vector_ELENGTH)
    -  [<span class="move-vis move-vis-const">const</span> `WORD_SIZE`](#std_bit_vector_WORD_SIZE)
    -  [<span class="move-vis move-vis-const">const</span> `MAX_SIZE`](#std_bit_vector_MAX_SIZE)


<pre><code></code></pre>



<a name="@Module_Functions_0"></a>

## Module Functions


<a name="std_bit_vector_new"></a>

### <span class="move-vis move-vis-public">pub</span> `new`



<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_new">new</a>(<a href="../std/bit_vector.md#std_bit_vector_length">length</a>: <a href="../std/u64.md#std_u64">u64</a>): <a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_new">new</a>(<a href="../std/bit_vector.md#std_bit_vector_length">length</a>: <a href="../std/u64.md#std_u64">u64</a>): <a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a> {
    <b>assert</b>!(<a href="../std/bit_vector.md#std_bit_vector_length">length</a> &gt; 0, <a href="../std/bit_vector.md#std_bit_vector_ELENGTH">ELENGTH</a>);
    <b>assert</b>!(<a href="../std/bit_vector.md#std_bit_vector_length">length</a> &lt; <a href="../std/bit_vector.md#std_bit_vector_MAX_SIZE">MAX_SIZE</a>, <a href="../std/bit_vector.md#std_bit_vector_ELENGTH">ELENGTH</a>);
    <b>let</b> <b>mut</b> counter = 0;
    <b>let</b> <b>mut</b> bit_field = <a href="../std/vector.md#std_vector_empty">vector::empty</a>();
    <b>while</b> (counter &lt; <a href="../std/bit_vector.md#std_bit_vector_length">length</a>) {
        bit_field.push_back(<b>false</b>);
        counter = counter + 1;
    };
    <a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a> {
        <a href="../std/bit_vector.md#std_bit_vector_length">length</a>,
        bit_field,
    }
}
</code></pre>



</details>

<a name="@Structs_1"></a>

## Structs


<a name="std_bit_vector_BitVector"></a>

### <span class="move-vis move-vis-struct">struct</span> `BitVector`



<pre><code><b>public</b> <b>struct</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a> <b>has</b> <b>copy</b>, drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code><a href="../std/bit_vector.md#std_bit_vector_length">length</a>: <a href="../std/u64.md#std_u64">u64</a></code>
</dt>
<dd>
</dd>
<dt>
<code>bit_field: <a href="../std/vector.md#std_vector">vector</a>&lt;bool&gt;</code>
</dt>
<dd>
</dd>
</dl>


</details>

<a name="std_bit_vector_is_index_set"></a>

#### <span class="move-vis move-vis-public">pub</span> `is_index_set`

Return the value of the bit at <code>bit_index</code> in the <code>bitvector</code>. <code><b>true</b></code>
represents "1" and <code><b>false</b></code> represents a 0


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_is_index_set">is_index_set</a>(bitvector: &<a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>, bit_index: <a href="../std/u64.md#std_u64">u64</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_is_index_set">is_index_set</a>(bitvector: &<a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a>, bit_index: <a href="../std/u64.md#std_u64">u64</a>): bool {
    <b>assert</b>!(bit_index &lt; bitvector.bit_field.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>(), <a href="../std/bit_vector.md#std_bit_vector_EINDEX">EINDEX</a>);
    bitvector.bit_field[bit_index]
}
</code></pre>



</details>

<a name="std_bit_vector_length"></a>

#### <span class="move-vis move-vis-public">pub</span> `length`

Return the length (number of usable bits) of this bitvector


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_length">length</a>(bitvector: &<a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>): <a href="../std/u64.md#std_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_length">length</a>(bitvector: &<a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a>): <a href="../std/u64.md#std_u64">u64</a> {
    bitvector.bit_field.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>()
}
</code></pre>



</details>

<a name="std_bit_vector_longest_set_sequence_starting_at"></a>

#### <span class="move-vis move-vis-public">pub</span> `longest_set_sequence_starting_at`

Returns the length of the longest sequence of set bits starting at (and
including) <code>start_index</code> in the <code>bitvector</code>. If there is no such
sequence, then <code>0</code> is returned.


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_longest_set_sequence_starting_at">longest_set_sequence_starting_at</a>(bitvector: &<a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>, start_index: <a href="../std/u64.md#std_u64">u64</a>): <a href="../std/u64.md#std_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_longest_set_sequence_starting_at">longest_set_sequence_starting_at</a>(bitvector: &<a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a>, start_index: <a href="../std/u64.md#std_u64">u64</a>): <a href="../std/u64.md#std_u64">u64</a> {
    <b>assert</b>!(start_index &lt; bitvector.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>, <a href="../std/bit_vector.md#std_bit_vector_EINDEX">EINDEX</a>);
    <b>let</b> <b>mut</b> index = start_index;
    // Find the greatest index in the <a href="../std/vector.md#std_vector">vector</a> such that all indices less than it are <a href="../std/bit_vector.md#std_bit_vector_set">set</a>.
    <b>while</b> (index &lt; bitvector.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>) {
        <b>if</b> (!bitvector.<a href="../std/bit_vector.md#std_bit_vector_is_index_set">is_index_set</a>(index)) <b>break</b>;
        index = index + 1;
    };
    index - start_index
}
</code></pre>



</details>

<a name="std_bit_vector_set"></a>

#### <span class="move-vis move-vis-public">pub</span> `set`

Set the bit at <code>bit_index</code> in the <code>bitvector</code> regardless of its previous state.


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_set">set</a>(bitvector: &<b>mut</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>, bit_index: <a href="../std/u64.md#std_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_set">set</a>(bitvector: &<b>mut</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a>, bit_index: <a href="../std/u64.md#std_u64">u64</a>) {
    <b>assert</b>!(bit_index &lt; bitvector.bit_field.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>(), <a href="../std/bit_vector.md#std_bit_vector_EINDEX">EINDEX</a>);
    <b>let</b> x = &<b>mut</b> bitvector.bit_field[bit_index];
    *x = <b>true</b>;
}
</code></pre>



</details>

<a name="std_bit_vector_shift_left"></a>

#### <span class="move-vis move-vis-public">pub</span> `shift_left`

Shift the <code>bitvector</code> left by <code>amount</code>. If <code>amount</code> is greater than the
bitvector's length the bitvector will be zeroed out.


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_shift_left">shift_left</a>(bitvector: &<b>mut</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>, amount: <a href="../std/u64.md#std_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_shift_left">shift_left</a>(bitvector: &<b>mut</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a>, amount: <a href="../std/u64.md#std_u64">u64</a>) {
    <b>if</b> (amount &gt;= bitvector.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>) {
        <b>let</b> len = bitvector.bit_field.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>();
        <b>let</b> <b>mut</b> i = 0;
        <b>while</b> (i &lt; len) {
            <b>let</b> elem = &<b>mut</b> bitvector.bit_field[i];
            *elem = <b>false</b>;
            i = i + 1;
        };
    } <b>else</b> {
        <b>let</b> <b>mut</b> i = amount;
        <b>while</b> (i &lt; bitvector.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>) {
            <b>if</b> (bitvector.<a href="../std/bit_vector.md#std_bit_vector_is_index_set">is_index_set</a>(i)) bitvector.<a href="../std/bit_vector.md#std_bit_vector_set">set</a>(i - amount)
            <b>else</b> bitvector.<a href="../std/bit_vector.md#std_bit_vector_unset">unset</a>(i - amount);
            i = i + 1;
        };
        i = bitvector.<a href="../std/bit_vector.md#std_bit_vector_length">length</a> - amount;
        <b>while</b> (i &lt; bitvector.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>) {
            <a href="../std/bit_vector.md#std_bit_vector_unset">unset</a>(bitvector, i);
            i = i + 1;
        };
    }
}
</code></pre>



</details>

<a name="std_bit_vector_unset"></a>

#### <span class="move-vis move-vis-public">pub</span> `unset`

Unset the bit at <code>bit_index</code> in the <code>bitvector</code> regardless of its previous state.


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_unset">unset</a>(bitvector: &<b>mut</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">std::bit_vector::BitVector</a>, bit_index: <a href="../std/u64.md#std_u64">u64</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/bit_vector.md#std_bit_vector_unset">unset</a>(bitvector: &<b>mut</b> <a href="../std/bit_vector.md#std_bit_vector_BitVector">BitVector</a>, bit_index: <a href="../std/u64.md#std_u64">u64</a>) {
    <b>assert</b>!(bit_index &lt; bitvector.bit_field.<a href="../std/bit_vector.md#std_bit_vector_length">length</a>(), <a href="../std/bit_vector.md#std_bit_vector_EINDEX">EINDEX</a>);
    <b>let</b> x = &<b>mut</b> bitvector.bit_field[bit_index];
    *x = <b>false</b>;
}
</code></pre>



</details>

<a name="@Constants_2"></a>

## Constants


<a name="std_bit_vector_EINDEX"></a>

### <span class="move-vis move-vis-error">err</span> `EINDEX`

The provided index is out of bounds


<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_EINDEX">EINDEX</a>: <a href="../std/u64.md#std_u64">u64</a> = 131072;
</code></pre>



<a name="std_bit_vector_ELENGTH"></a>

### <span class="move-vis move-vis-error">err</span> `ELENGTH`

An invalid length of bitvector was given


<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_ELENGTH">ELENGTH</a>: <a href="../std/u64.md#std_u64">u64</a> = 131073;
</code></pre>



<a name="std_bit_vector_WORD_SIZE"></a>

### <span class="move-vis move-vis-const">const</span> `WORD_SIZE`



<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_WORD_SIZE">WORD_SIZE</a>: <a href="../std/u64.md#std_u64">u64</a> = 1;
</code></pre>



<a name="std_bit_vector_MAX_SIZE"></a>

### <span class="move-vis move-vis-const">const</span> `MAX_SIZE`

The maximum allowed bitvector size


<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_MAX_SIZE">MAX_SIZE</a>: <a href="../std/u64.md#std_u64">u64</a> = 1024;
</code></pre>


[//]: # ("File containing references which can be used from documentation")
