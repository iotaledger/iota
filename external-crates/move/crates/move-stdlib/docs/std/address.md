
<a name="std_address"></a>

# Module `std::address`

Provides a way to get address length since it's a
platform-specific parameter.


-  [Public Functions](#@Public_Functions_0)
    -  [`length`](#std_address_length)


<pre><code></code></pre>



<a name="@Public_Functions_0"></a>

## Public Functions


<a name="std_address_length"></a>

### `length`

Should be converted to a native function.
Current implementation only works for IOTA.


<pre><code><b>public</b> <b>fun</b> <a href="../std/address.md#std_address_length">length</a>(): <a href="../std/u64.md#std_u64">u64</a>
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>fun</b> <a href="../std/address.md#std_address_length">length</a>(): <a href="../std/u64.md#std_u64">u64</a> {
    32
}
</code></pre>



</details>


[//]: # ("File containing references which can be used from documentation")
