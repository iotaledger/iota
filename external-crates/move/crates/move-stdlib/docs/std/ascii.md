
<a name="std_ascii"></a>

# Module `std::ascii`

The <code>ASCII</code> module defines basic string and char newtypes in Move that verify
that characters are valid ASCII, and that strings consist of only valid ASCII characters.


-  [Module Functions](#@Module_Functions_0)
    -  [<span class="move-vis move-vis-public">pub</span> `char`](#std_ascii_char)
    -  [<span class="move-vis move-vis-public">pub</span> `is_printable_char`](#std_ascii_is_printable_char)
    -  [<span class="move-vis move-vis-public">pub</span> `is_valid_char`](#std_ascii_is_valid_char)
    -  [<span class="move-vis move-vis-public">pub</span> `string`](#std_ascii_string)
    -  [<span class="move-vis move-vis-public">pub</span> `try_string`](#std_ascii_try_string)
    -  [<span class="move-vis move-vis-private">prv</span> `char_to_lowercase`](#std_ascii_char_to_lowercase)
    -  [<span class="move-vis move-vis-private">prv</span> `char_to_uppercase`](#std_ascii_char_to_uppercase)
-  [Structs](#@Structs_1)
    -  [<span class="move-vis move-vis-struct">struct</span> `String`](#std_ascii_String)
        -  [<span class="move-vis move-vis-public">pub</span> `all_characters_printable`](#std_ascii_all_characters_printable)
        -  [<span class="move-vis move-vis-public">pub</span> `append`](#std_ascii_append)
        -  [<span class="move-vis move-vis-public">pub</span> `as_bytes`](#std_ascii_as_bytes)
        -  [<span class="move-vis move-vis-public">pub</span> `index_of`](#std_ascii_index_of)
        -  [<span class="move-vis move-vis-public">pub</span> `insert`](#std_ascii_insert)
        -  [<span class="move-vis move-vis-public">pub</span> `into_bytes`](#std_ascii_into_bytes)
        -  [<span class="move-vis move-vis-public">pub</span> `is_empty`](#std_ascii_is_empty)
        -  [<span class="move-vis move-vis-public">pub</span> `length`](#std_ascii_length)
        -  [<span class="move-vis move-vis-public">pub</span> `pop_char`](#std_ascii_pop_char)
        -  [<span class="move-vis move-vis-public">pub</span> `push_char`](#std_ascii_push_char)
        -  [<span class="move-vis move-vis-public">pub</span> `substring`](#std_ascii_substring)
        -  [<span class="move-vis move-vis-public">pub</span> `to_lowercase`](#std_ascii_to_lowercase)
        -  [<span class="move-vis move-vis-public">pub</span> `to_uppercase`](#std_ascii_to_uppercase)
    -  [<span class="move-vis move-vis-struct">struct</span> `Char`](#std_ascii_Char)
        -  [<span class="move-vis move-vis-public">pub</span> `byte`](#std_ascii_byte)
-  [Constants](#@Constants_2)
    -  [<span class="move-vis move-vis-error">err</span> `EInvalidASCIICharacter`](#std_ascii_EInvalidASCIICharacter)
    -  [<span class="move-vis move-vis-error">err</span> `EInvalidIndex`](#std_ascii_EInvalidIndex)


<pre><code><b>use</b> <a href="../std/option.md#std_option">std::option</a>;
<b>use</b> <a href="../std/vector.md#std_vector">std::vector</a>;
</code></pre>



<a name="@Module_Functions_0"></a>

## Module Functions


<a name="std_ascii_char"></a>

### <span class="move-vis move-vis-public">pub</span> `char`

Convert a <code><a href="../std/ascii.md#std_ascii_byte">byte</a></code> into a <code><a href="../std/ascii.md#std_ascii_Char">Char</a></code> that is checked to make sure it is valid ASCII.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_char">char</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): <a href="../std/ascii.md#std_ascii_Char">std::ascii::Char</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_char">char</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): <a href="../std/ascii.md#std_ascii_Char">Char</a> {
    <b>assert</b>!(<a href="../std/ascii.md#std_ascii_is_valid_char">is_valid_char</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>), <a href="../std/ascii.md#std_ascii_EInvalidASCIICharacter">EInvalidASCIICharacter</a>);
    <a href="../std/ascii.md#std_ascii_Char">Char</a> { <a href="../std/ascii.md#std_ascii_byte">byte</a> }
}
</code></pre>



</details>

<a name="std_ascii_is_printable_char"></a>

### <span class="move-vis move-vis-public">pub</span> `is_printable_char`

Returns <code><b>true</b></code> if <code><a href="../std/ascii.md#std_ascii_byte">byte</a></code> is a printable ASCII character.
Returns <code><b>false</b></code> otherwise.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_is_printable_char">is_printable_char</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_is_printable_char">is_printable_char</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): bool {
    <a href="../std/ascii.md#std_ascii_byte">byte</a> &gt;= 0x20 && // Disallow metacharacters
        <a href="../std/ascii.md#std_ascii_byte">byte</a> &lt;= 0x7E // Don't allow DEL metacharacter
}
</code></pre>



</details>

<a name="std_ascii_is_valid_char"></a>

### <span class="move-vis move-vis-public">pub</span> `is_valid_char`

Returns <code><b>true</b></code> if <code>b</code> is a valid ASCII character.
Returns <code><b>false</b></code> otherwise.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_is_valid_char">is_valid_char</a>(b: <a href="../std/u8.md#std_u8">u8</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_is_valid_char">is_valid_char</a>(b: <a href="../std/u8.md#std_u8">u8</a>): bool {
    b &lt;= 0x7F
}
</code></pre>



</details>

<a name="std_ascii_string"></a>

### <span class="move-vis move-vis-public">pub</span> `string`

Convert a vector of bytes <code>bytes</code> into an <code><a href="../std/ascii.md#std_ascii_String">String</a></code>. Aborts if
<code>bytes</code> contains non-ASCII characters.


<pre><code><b>public</b> <b>fun</b> <a href="../std/string.md#std_string">string</a>(bytes: <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;): <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/string.md#std_string">string</a>(bytes: <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;): <a href="../std/ascii.md#std_ascii_String">String</a> {
    <b>let</b> x = <a href="../std/ascii.md#std_ascii_try_string">try_string</a>(bytes);
    <b>assert</b>!(x.is_some(), <a href="../std/ascii.md#std_ascii_EInvalidASCIICharacter">EInvalidASCIICharacter</a>);
    x.destroy_some()
}
</code></pre>



</details>

<a name="std_ascii_try_string"></a>

### <span class="move-vis move-vis-public">pub</span> `try_string`

Convert a vector of bytes <code>bytes</code> into an <code><a href="../std/ascii.md#std_ascii_String">String</a></code>. Returns
<code>Some(&lt;ascii_string&gt;)</code> if the <code>bytes</code> contains all valid ASCII
characters. Otherwise returns <code>None</code>.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_try_string">try_string</a>(bytes: <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;): <a href="../std/option.md#std_option_Option">std::option::Option</a>&lt;<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_try_string">try_string</a>(bytes: <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;): Option&lt;<a href="../std/ascii.md#std_ascii_String">String</a>&gt; {
    <b>let</b> is_valid = bytes.all!(|<a href="../std/ascii.md#std_ascii_byte">byte</a>| <a href="../std/ascii.md#std_ascii_is_valid_char">is_valid_char</a>(*<a href="../std/ascii.md#std_ascii_byte">byte</a>));
    <b>if</b> (is_valid) <a href="../std/option.md#std_option_some">option::some</a>(<a href="../std/ascii.md#std_ascii_String">String</a> { bytes })
    <b>else</b> <a href="../std/option.md#std_option_none">option::none</a>()
}
</code></pre>



</details>

<a name="std_ascii_char_to_lowercase"></a>

### <span class="move-vis move-vis-private">prv</span> `char_to_lowercase`

Convert a <code><a href="../std/ascii.md#std_ascii_char">char</a></code> to its lowercase equivalent.


<pre><code><b>fun</b> <a href="../std/ascii.md#std_ascii_char_to_lowercase">char_to_lowercase</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): <a href="../std/u8.md#std_u8">u8</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../std/ascii.md#std_ascii_char_to_lowercase">char_to_lowercase</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): <a href="../std/u8.md#std_u8">u8</a> {
    <b>if</b> (<a href="../std/ascii.md#std_ascii_byte">byte</a> &gt;= 0x41 && <a href="../std/ascii.md#std_ascii_byte">byte</a> &lt;= 0x5A) <a href="../std/ascii.md#std_ascii_byte">byte</a> + 0x20
    <b>else</b> <a href="../std/ascii.md#std_ascii_byte">byte</a>
}
</code></pre>



</details>

<a name="std_ascii_char_to_uppercase"></a>

### <span class="move-vis move-vis-private">prv</span> `char_to_uppercase`

Convert a <code><a href="../std/ascii.md#std_ascii_char">char</a></code> to its lowercase equivalent.


<pre><code><b>fun</b> <a href="../std/ascii.md#std_ascii_char_to_uppercase">char_to_uppercase</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): <a href="../std/u8.md#std_u8">u8</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>fun</b> <a href="../std/ascii.md#std_ascii_char_to_uppercase">char_to_uppercase</a>(<a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a>): <a href="../std/u8.md#std_u8">u8</a> {
    <b>if</b> (<a href="../std/ascii.md#std_ascii_byte">byte</a> &gt;= 0x61 && <a href="../std/ascii.md#std_ascii_byte">byte</a> &lt;= 0x7A) <a href="../std/ascii.md#std_ascii_byte">byte</a> - 0x20
    <b>else</b> <a href="../std/ascii.md#std_ascii_byte">byte</a>
}
</code></pre>



</details>

<a name="@Structs_1"></a>

## Structs


<a name="std_ascii_String"></a>

### <span class="move-vis move-vis-struct">struct</span> `String`

The <code><a href="../std/ascii.md#std_ascii_String">String</a></code> struct holds a vector of bytes that all represent
valid ASCII characters. Note that these ASCII characters may not all
be printable. To determine if a <code><a href="../std/ascii.md#std_ascii_String">String</a></code> contains only "printable"
characters you should use the <code><a href="../std/ascii.md#std_ascii_all_characters_printable">all_characters_printable</a></code> predicate
defined in this module.


<pre><code><b>public</b> <b>struct</b> <a href="../std/ascii.md#std_ascii_String">String</a> <b>has</b> <b>copy</b>, drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code>bytes: <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;</code>
</dt>
<dd>
</dd>
</dl>


</details>

<a name="std_ascii_all_characters_printable"></a>

#### <span class="move-vis move-vis-public">pub</span> `all_characters_printable`

Returns <code><b>true</b></code> if all characters in <code><a href="../std/string.md#std_string">string</a></code> are printable characters
Returns <code><b>false</b></code> otherwise. Not all <code><a href="../std/ascii.md#std_ascii_String">String</a></code>s are printable strings.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_all_characters_printable">all_characters_printable</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_all_characters_printable">all_characters_printable</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>): bool {
    <a href="../std/string.md#std_string">string</a>.bytes.all!(|<a href="../std/ascii.md#std_ascii_byte">byte</a>| <a href="../std/ascii.md#std_ascii_is_printable_char">is_printable_char</a>(*<a href="../std/ascii.md#std_ascii_byte">byte</a>))
}
</code></pre>



</details>

<a name="std_ascii_append"></a>

#### <span class="move-vis move-vis-public">pub</span> `append`

Append the <code>other</code> string to the end of <code><a href="../std/string.md#std_string">string</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_append">append</a>(<a href="../std/string.md#std_string">string</a>: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>, other: <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_append">append</a>(<a href="../std/string.md#std_string">string</a>: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">String</a>, other: <a href="../std/ascii.md#std_ascii_String">String</a>) {
    <a href="../std/string.md#std_string">string</a>.bytes.<a href="../std/ascii.md#std_ascii_append">append</a>(other.<a href="../std/ascii.md#std_ascii_into_bytes">into_bytes</a>())
}
</code></pre>



</details>

<a name="std_ascii_as_bytes"></a>

#### <span class="move-vis move-vis-public">pub</span> `as_bytes`

Get the inner bytes of the <code><a href="../std/string.md#std_string">string</a></code> as a reference


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_as_bytes">as_bytes</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): &<a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_as_bytes">as_bytes</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>): &<a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt; {
    &<a href="../std/string.md#std_string">string</a>.bytes
}
</code></pre>



</details>

<a name="std_ascii_index_of"></a>

#### <span class="move-vis move-vis-public">pub</span> `index_of`

Computes the index of the first occurrence of the <code>substr</code> in the <code><a href="../std/string.md#std_string">string</a></code>.
Returns the length of the <code><a href="../std/string.md#std_string">string</a></code> if the <code>substr</code> is not found.
Returns 0 if the <code>substr</code> is empty.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_index_of">index_of</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>, substr: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): <a href="../std/u64.md#std_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_index_of">index_of</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>, substr: &<a href="../std/ascii.md#std_ascii_String">String</a>): <a href="../std/u64.md#std_u64">u64</a> {
    <b>let</b> <b>mut</b> i = 0;
    <b>let</b> (n, m) = (<a href="../std/string.md#std_string">string</a>.<a href="../std/ascii.md#std_ascii_length">length</a>(), substr.<a href="../std/ascii.md#std_ascii_length">length</a>());
    <b>if</b> (n &lt; m) <b>return</b> n;
    <b>while</b> (i &lt;= n - m) {
        <b>let</b> <b>mut</b> j = 0;
        <b>while</b> (j &lt; m && <a href="../std/string.md#std_string">string</a>.bytes[i + j] == substr.bytes[j]) j = j + 1;
        <b>if</b> (j == m) <b>return</b> i;
        i = i + 1;
    };
    n
}
</code></pre>



</details>

<a name="std_ascii_insert"></a>

#### <span class="move-vis move-vis-public">pub</span> `insert`

Insert the <code>other</code> string at the <code>at</code> index of <code><a href="../std/string.md#std_string">string</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_insert">insert</a>(s: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>, at: <a href="../std/u64.md#std_u64">u64</a>, o: <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_insert">insert</a>(s: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">String</a>, at: <a href="../std/u64.md#std_u64">u64</a>, o: <a href="../std/ascii.md#std_ascii_String">String</a>) {
    <b>assert</b>!(at &lt;= s.<a href="../std/ascii.md#std_ascii_length">length</a>(), <a href="../std/ascii.md#std_ascii_EInvalidIndex">EInvalidIndex</a>);
    o.<a href="../std/ascii.md#std_ascii_into_bytes">into_bytes</a>().destroy!(|e| s.bytes.<a href="../std/ascii.md#std_ascii_insert">insert</a>(e, at));
}
</code></pre>



</details>

<a name="std_ascii_into_bytes"></a>

#### <span class="move-vis move-vis-public">pub</span> `into_bytes`

Unpack the <code><a href="../std/string.md#std_string">string</a></code> to get its backing bytes


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_into_bytes">into_bytes</a>(<a href="../std/string.md#std_string">string</a>: <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_into_bytes">into_bytes</a>(<a href="../std/string.md#std_string">string</a>: <a href="../std/ascii.md#std_ascii_String">String</a>): <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt; {
    <b>let</b> <a href="../std/ascii.md#std_ascii_String">String</a> { bytes } = <a href="../std/string.md#std_string">string</a>;
    bytes
}
</code></pre>



</details>

<a name="std_ascii_is_empty"></a>

#### <span class="move-vis move-vis-public">pub</span> `is_empty`

Returns <code><b>true</b></code> if <code><a href="../std/string.md#std_string">string</a></code> is empty.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_is_empty">is_empty</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): bool
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_is_empty">is_empty</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>): bool {
    <a href="../std/string.md#std_string">string</a>.bytes.<a href="../std/ascii.md#std_ascii_is_empty">is_empty</a>()
}
</code></pre>



</details>

<a name="std_ascii_length"></a>

#### <span class="move-vis move-vis-public">pub</span> `length`

Returns the length of the <code><a href="../std/string.md#std_string">string</a></code> in bytes.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_length">length</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): <a href="../std/u64.md#std_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_length">length</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>): <a href="../std/u64.md#std_u64">u64</a> {
    <a href="../std/string.md#std_string">string</a>.<a href="../std/ascii.md#std_ascii_as_bytes">as_bytes</a>().<a href="../std/ascii.md#std_ascii_length">length</a>()
}
</code></pre>



</details>

<a name="std_ascii_pop_char"></a>

#### <span class="move-vis move-vis-public">pub</span> `pop_char`

Pop a <code><a href="../std/ascii.md#std_ascii_Char">Char</a></code> from the end of the <code><a href="../std/string.md#std_string">string</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_pop_char">pop_char</a>(<a href="../std/string.md#std_string">string</a>: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): <a href="../std/ascii.md#std_ascii_Char">std::ascii::Char</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_pop_char">pop_char</a>(<a href="../std/string.md#std_string">string</a>: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">String</a>): <a href="../std/ascii.md#std_ascii_Char">Char</a> {
    <a href="../std/ascii.md#std_ascii_Char">Char</a> { <a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/string.md#std_string">string</a>.bytes.pop_back() }
}
</code></pre>



</details>

<a name="std_ascii_push_char"></a>

#### <span class="move-vis move-vis-public">pub</span> `push_char`

Push a <code><a href="../std/ascii.md#std_ascii_Char">Char</a></code> to the end of the <code><a href="../std/string.md#std_string">string</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_push_char">push_char</a>(<a href="../std/string.md#std_string">string</a>: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>, <a href="../std/ascii.md#std_ascii_char">char</a>: <a href="../std/ascii.md#std_ascii_Char">std::ascii::Char</a>)
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_push_char">push_char</a>(<a href="../std/string.md#std_string">string</a>: &<b>mut</b> <a href="../std/ascii.md#std_ascii_String">String</a>, <a href="../std/ascii.md#std_ascii_char">char</a>: <a href="../std/ascii.md#std_ascii_Char">Char</a>) {
    <a href="../std/string.md#std_string">string</a>.bytes.push_back(<a href="../std/ascii.md#std_ascii_char">char</a>.<a href="../std/ascii.md#std_ascii_byte">byte</a>);
}
</code></pre>



</details>

<a name="std_ascii_substring"></a>

#### <span class="move-vis move-vis-public">pub</span> `substring`

Copy the slice of the <code><a href="../std/string.md#std_string">string</a></code> from <code>i</code> to <code>j</code> into a new <code><a href="../std/ascii.md#std_ascii_String">String</a></code>.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_substring">substring</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>, i: <a href="../std/u64.md#std_u64">u64</a>, j: <a href="../std/u64.md#std_u64">u64</a>): <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_substring">substring</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>, i: <a href="../std/u64.md#std_u64">u64</a>, j: <a href="../std/u64.md#std_u64">u64</a>): <a href="../std/ascii.md#std_ascii_String">String</a> {
    <b>assert</b>!(i &lt;= j && j &lt;= <a href="../std/string.md#std_string">string</a>.<a href="../std/ascii.md#std_ascii_length">length</a>(), <a href="../std/ascii.md#std_ascii_EInvalidIndex">EInvalidIndex</a>);
    <b>let</b> <b>mut</b> bytes = <a href="../std/vector.md#std_vector">vector</a>[];
    i.range_do!(j, |i| bytes.push_back(<a href="../std/string.md#std_string">string</a>.bytes[i]));
    <a href="../std/ascii.md#std_ascii_String">String</a> { bytes }
}
</code></pre>



</details>

<a name="std_ascii_to_lowercase"></a>

#### <span class="move-vis move-vis-public">pub</span> `to_lowercase`

Convert a <code><a href="../std/string.md#std_string">string</a></code> to its lowercase equivalent.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_to_lowercase">to_lowercase</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_to_lowercase">to_lowercase</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>): <a href="../std/ascii.md#std_ascii_String">String</a> {
    <b>let</b> bytes = <a href="../std/string.md#std_string">string</a>.<a href="../std/ascii.md#std_ascii_as_bytes">as_bytes</a>().map_ref!(|<a href="../std/ascii.md#std_ascii_byte">byte</a>| <a href="../std/ascii.md#std_ascii_char_to_lowercase">char_to_lowercase</a>(*<a href="../std/ascii.md#std_ascii_byte">byte</a>));
    <a href="../std/ascii.md#std_ascii_String">String</a> { bytes }
}
</code></pre>



</details>

<a name="std_ascii_to_uppercase"></a>

#### <span class="move-vis move-vis-public">pub</span> `to_uppercase`

Convert a <code><a href="../std/string.md#std_string">string</a></code> to its uppercase equivalent.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_to_uppercase">to_uppercase</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>): <a href="../std/ascii.md#std_ascii_String">std::ascii::String</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_to_uppercase">to_uppercase</a>(<a href="../std/string.md#std_string">string</a>: &<a href="../std/ascii.md#std_ascii_String">String</a>): <a href="../std/ascii.md#std_ascii_String">String</a> {
    <b>let</b> bytes = <a href="../std/string.md#std_string">string</a>.<a href="../std/ascii.md#std_ascii_as_bytes">as_bytes</a>().map_ref!(|<a href="../std/ascii.md#std_ascii_byte">byte</a>| <a href="../std/ascii.md#std_ascii_char_to_uppercase">char_to_uppercase</a>(*<a href="../std/ascii.md#std_ascii_byte">byte</a>));
    <a href="../std/ascii.md#std_ascii_String">String</a> { bytes }
}
</code></pre>



</details>

<a name="std_ascii_Char"></a>

### <span class="move-vis move-vis-struct">struct</span> `Char`

An ASCII character.


<pre><code><b>public</b> <b>struct</b> <a href="../std/ascii.md#std_ascii_Char">Char</a> <b>has</b> <b>copy</b>, drop, store
</code></pre>



<details>
<summary>Fields</summary>


<dl>
<dt>
<code><a href="../std/ascii.md#std_ascii_byte">byte</a>: <a href="../std/u8.md#std_u8">u8</a></code>
</dt>
<dd>
</dd>
</dl>


</details>

<a name="std_ascii_byte"></a>

#### <span class="move-vis move-vis-public">pub</span> `byte`

Unpack the <code><a href="../std/ascii.md#std_ascii_char">char</a></code> into its underlying bytes.


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_byte">byte</a>(<a href="../std/ascii.md#std_ascii_char">char</a>: <a href="../std/ascii.md#std_ascii_Char">std::ascii::Char</a>): <a href="../std/u8.md#std_u8">u8</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/ascii.md#std_ascii_byte">byte</a>(<a href="../std/ascii.md#std_ascii_char">char</a>: <a href="../std/ascii.md#std_ascii_Char">Char</a>): <a href="../std/u8.md#std_u8">u8</a> {
    <b>let</b> <a href="../std/ascii.md#std_ascii_Char">Char</a> { <a href="../std/ascii.md#std_ascii_byte">byte</a> } = <a href="../std/ascii.md#std_ascii_char">char</a>;
    <a href="../std/ascii.md#std_ascii_byte">byte</a>
}
</code></pre>



</details>

<a name="@Constants_2"></a>

## Constants


<a name="std_ascii_EInvalidASCIICharacter"></a>

### <span class="move-vis move-vis-error">err</span> `EInvalidASCIICharacter`

An invalid ASCII character was encountered when creating an ASCII string.


<pre><code><b>const</b> <a href="../std/ascii.md#std_ascii_EInvalidASCIICharacter">EInvalidASCIICharacter</a>: <a href="../std/u64.md#std_u64">u64</a> = 65536;
</code></pre>



<a name="std_ascii_EInvalidIndex"></a>

### <span class="move-vis move-vis-error">err</span> `EInvalidIndex`

An invalid index was encountered when creating a substring.


<pre><code><b>const</b> <a href="../std/ascii.md#std_ascii_EInvalidIndex">EInvalidIndex</a>: <a href="../std/u64.md#std_u64">u64</a> = 65537;
</code></pre>


[//]: # ("File containing references which can be used from documentation")
