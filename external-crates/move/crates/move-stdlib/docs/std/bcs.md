
<a name="std_bcs"></a>

# Module `std::bcs`

Utility for converting a Move value to its binary representation in BCS (Binary Canonical
Serialization). BCS is the binary encoding for Move resources and other non-module values
published on-chain. See https://github.com/diem/bcs#binary-canonical-serialization-bcs for more
details on BCS.


-  [Module Functions](#@Module_Functions_0)
    -  [<span class="move-vis move-vis-public">pub</span> `to_bytes`](#std_bcs_to_bytes)


<pre><code></code></pre>



<a name="@Module_Functions_0"></a>

## Module Functions


<a name="std_bcs_to_bytes"></a>

### <span class="move-vis move-vis-public">pub</span> `to_bytes`

Return the binary representation of <code>v</code> in BCS (Binary Canonical Serialization) format


<pre><code><b>public</b> <b>fun</b> <a href="../std/bcs.md#std_bcs_to_bytes">to_bytes</a>&lt;MoveValue&gt;(v: &MoveValue): <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;
</code></pre>



<details>
<summary>Implementation</summary>


<pre><code><b>public</b> <b>native</b> <b>fun</b> <a href="../std/bcs.md#std_bcs_to_bytes">to_bytes</a>&lt;MoveValue&gt;(v: &MoveValue): <a href="../std/vector.md#std_vector">vector</a>&lt;<a href="../std/u8.md#std_u8">u8</a>&gt;;
</code></pre>



</details>


[//]: # ("File containing references which can be used from documentation")
