
<a name="std_bit_vector"></a>

# Module `std::bit_vector`



-  [Structs](#@Structs_0)
    -  [`BitVector`](#std_bit_vector_BitVector)
        -  [`is_index_set` <span class="move-vis move-vis-public">pub</span>](#std_bit_vector_is_index_set)
        -  [`length` <span class="move-vis move-vis-public">pub</span>](#std_bit_vector_length)
        -  [`longest_set_sequence_starting_at` <span class="move-vis move-vis-public">pub</span>](#std_bit_vector_longest_set_sequence_starting_at)
        -  [`set` <span class="move-vis move-vis-public">pub</span>](#std_bit_vector_set)
        -  [`shift_left` <span class="move-vis move-vis-public">pub</span>](#std_bit_vector_shift_left)
        -  [`unset` <span class="move-vis move-vis-public">pub</span>](#std_bit_vector_unset)
-  [Constants](#@Constants_1)
    -  [`EINDEX` <span class="move-vis move-vis-error">err</span>](#std_bit_vector_EINDEX)
    -  [`ELENGTH` <span class="move-vis move-vis-error">err</span>](#std_bit_vector_ELENGTH)
-  [Module Functions](#@Module_Functions_2)
    -  [`new` <span class="move-vis move-vis-public">pub</span> <span class="move-vis move-vis-module">module</span>](#std_bit_vector_new)


<pre><code></code></pre>



<a name="@Structs_0"></a>

## Structs


<a name="std_bit_vector_BitVector"></a>

### `BitVector`



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

#### `is_index_set` <span class="move-vis move-vis-public">pub</span>

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

#### `length` <span class="move-vis move-vis-public">pub</span>

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

#### `longest_set_sequence_starting_at` <span class="move-vis move-vis-public">pub</span>

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

#### `set` <span class="move-vis move-vis-public">pub</span>

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

#### `shift_left` <span class="move-vis move-vis-public">pub</span>

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

#### `unset` <span class="move-vis move-vis-public">pub</span>

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

<a name="@Constants_1"></a>

## Constants


<a name="std_bit_vector_EINDEX"></a>

### `EINDEX` <span class="move-vis move-vis-error">err</span>

The provided index is out of bounds


<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_EINDEX">EINDEX</a>: <a href="../std/u64.md#std_u64">u64</a> = 131072;
</code></pre>



<a name="std_bit_vector_ELENGTH"></a>

### `ELENGTH` <span class="move-vis move-vis-error">err</span>

An invalid length of bitvector was given


<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_ELENGTH">ELENGTH</a>: <a href="../std/u64.md#std_u64">u64</a> = 131073;
</code></pre>



<a name="std_bit_vector_WORD_SIZE"></a>



<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_WORD_SIZE">WORD_SIZE</a>: <a href="../std/u64.md#std_u64">u64</a> = 1;
</code></pre>



<a name="std_bit_vector_MAX_SIZE"></a>

The maximum allowed bitvector size


<pre><code><b>const</b> <a href="../std/bit_vector.md#std_bit_vector_MAX_SIZE">MAX_SIZE</a>: <a href="../std/u64.md#std_u64">u64</a> = 1024;
</code></pre>



<a name="@Module_Functions_2"></a>

## Module Functions


<a name="std_bit_vector_new"></a>

### `new` <span class="move-vis move-vis-public">pub</span> <span class="move-vis move-vis-module">module</span>



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


[//]: # ("File containing references which can be used from documentation")
