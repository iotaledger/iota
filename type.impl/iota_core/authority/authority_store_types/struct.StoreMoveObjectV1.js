(function() {
    var type_impls = Object.fromEntries([["iota_core",[["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Clone-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-Clone-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/clone/trait.Clone.html\" title=\"trait core::clone::Clone\">Clone</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.clone\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#method.clone\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/clone/trait.Clone.html#tymethod.clone\" class=\"fn\">clone</a>(&amp;self) -&gt; <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h4></section></summary><div class='docblock'>Returns a copy of the value. <a href=\"https://doc.rust-lang.org/1.82.0/core/clone/trait.Clone.html#tymethod.clone\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.clone_from\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/1.82.0/src/core/clone.rs.html#174\">source</a></span><a href=\"#method.clone_from\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/clone/trait.Clone.html#method.clone_from\" class=\"fn\">clone_from</a>(&amp;mut self, source: &amp;Self)</h4></section></summary><div class='docblock'>Performs copy-assignment from <code>source</code>. <a href=\"https://doc.rust-lang.org/1.82.0/core/clone/trait.Clone.html#method.clone_from\">Read more</a></div></details></div></details>","Clone","iota_core::authority::authority_store_types::StoreMoveObject"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Debug-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-Debug-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/fmt/trait.Debug.html\" title=\"trait core::fmt::Debug\">Debug</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.fmt\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#method.fmt\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/fmt/trait.Debug.html#tymethod.fmt\" class=\"fn\">fmt</a>(&amp;self, f: &amp;mut <a class=\"struct\" href=\"https://doc.rust-lang.org/1.82.0/core/fmt/struct.Formatter.html\" title=\"struct core::fmt::Formatter\">Formatter</a>&lt;'_&gt;) -&gt; <a class=\"type\" href=\"https://doc.rust-lang.org/1.82.0/core/fmt/type.Result.html\" title=\"type core::fmt::Result\">Result</a></h4></section></summary><div class='docblock'>Formats the value using the given formatter. <a href=\"https://doc.rust-lang.org/1.82.0/core/fmt/trait.Debug.html#tymethod.fmt\">Read more</a></div></details></div></details>","Debug","iota_core::authority::authority_store_types::StoreMoveObject"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Deserialize%3C'de%3E-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-Deserialize%3C'de%3E-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl&lt;'de&gt; <a class=\"trait\" href=\"https://docs.rs/serde/1.0.210/serde/de/trait.Deserialize.html\" title=\"trait serde::de::Deserialize\">Deserialize</a>&lt;'de&gt; for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.deserialize\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#method.deserialize\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://docs.rs/serde/1.0.210/serde/de/trait.Deserialize.html#tymethod.deserialize\" class=\"fn\">deserialize</a>&lt;__D&gt;(__deserializer: __D) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.82.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;Self, __D::<a class=\"associatedtype\" href=\"https://docs.rs/serde/1.0.210/serde/de/trait.Deserializer.html#associatedtype.Error\" title=\"type serde::de::Deserializer::Error\">Error</a>&gt;<div class=\"where\">where\n    __D: <a class=\"trait\" href=\"https://docs.rs/serde/1.0.210/serde/de/trait.Deserializer.html\" title=\"trait serde::de::Deserializer\">Deserializer</a>&lt;'de&gt;,</div></h4></section></summary><div class='docblock'>Deserialize this value from the given Serde deserializer. <a href=\"https://docs.rs/serde/1.0.210/serde/de/trait.Deserialize.html#tymethod.deserialize\">Read more</a></div></details></div></details>","Deserialize<'de>","iota_core::authority::authority_store_types::StoreMoveObject"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Hash-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-Hash-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hash.html\" title=\"trait core::hash::Hash\">Hash</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.hash\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#method.hash\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hash.html#tymethod.hash\" class=\"fn\">hash</a>&lt;__H: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hasher.html\" title=\"trait core::hash::Hasher\">Hasher</a>&gt;(&amp;self, state: <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.82.0/std/primitive.reference.html\">&amp;mut __H</a>)</h4></section></summary><div class='docblock'>Feeds this value into the given <a href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hasher.html\" title=\"trait core::hash::Hasher\"><code>Hasher</code></a>. <a href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hash.html#tymethod.hash\">Read more</a></div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.hash_slice\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.3.0\">1.3.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/1.82.0/src/core/hash/mod.rs.html#235-237\">source</a></span><a href=\"#method.hash_slice\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hash.html#method.hash_slice\" class=\"fn\">hash_slice</a>&lt;H&gt;(data: &amp;[Self], state: <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.82.0/std/primitive.reference.html\">&amp;mut H</a>)<div class=\"where\">where\n    H: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hasher.html\" title=\"trait core::hash::Hasher\">Hasher</a>,\n    Self: <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/marker/trait.Sized.html\" title=\"trait core::marker::Sized\">Sized</a>,</div></h4></section></summary><div class='docblock'>Feeds a slice of this type into the given <a href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hasher.html\" title=\"trait core::hash::Hasher\"><code>Hasher</code></a>. <a href=\"https://doc.rust-lang.org/1.82.0/core/hash/trait.Hash.html#method.hash_slice\">Read more</a></div></details></div></details>","Hash","iota_core::authority::authority_store_types::StoreMoveObject"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-PartialEq-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-PartialEq-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/cmp/trait.PartialEq.html\" title=\"trait core::cmp::PartialEq\">PartialEq</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.eq\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#method.eq\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/cmp/trait.PartialEq.html#tymethod.eq\" class=\"fn\">eq</a>(&amp;self, other: &amp;<a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.82.0/std/primitive.bool.html\">bool</a></h4></section></summary><div class='docblock'>Tests for <code>self</code> and <code>other</code> values to be equal, and is used by <code>==</code>.</div></details><details class=\"toggle method-toggle\" open><summary><section id=\"method.ne\" class=\"method trait-impl\"><span class=\"rightside\"><span class=\"since\" title=\"Stable since Rust version 1.0.0\">1.0.0</span> · <a class=\"src\" href=\"https://doc.rust-lang.org/1.82.0/src/core/cmp.rs.html#261\">source</a></span><a href=\"#method.ne\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://doc.rust-lang.org/1.82.0/core/cmp/trait.PartialEq.html#method.ne\" class=\"fn\">ne</a>(&amp;self, other: <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.82.0/std/primitive.reference.html\">&amp;Rhs</a>) -&gt; <a class=\"primitive\" href=\"https://doc.rust-lang.org/1.82.0/std/primitive.bool.html\">bool</a></h4></section></summary><div class='docblock'>Tests for <code>!=</code>. The default implementation is almost always sufficient,\nand should not be overridden without very good reason.</div></details></div></details>","PartialEq","iota_core::authority::authority_store_types::StoreMoveObject"],["<details class=\"toggle implementors-toggle\" open><summary><section id=\"impl-Serialize-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-Serialize-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://docs.rs/serde/1.0.210/serde/ser/trait.Serialize.html\" title=\"trait serde::ser::Serialize\">Serialize</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section></summary><div class=\"impl-items\"><details class=\"toggle method-toggle\" open><summary><section id=\"method.serialize\" class=\"method trait-impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#method.serialize\" class=\"anchor\">§</a><h4 class=\"code-header\">fn <a href=\"https://docs.rs/serde/1.0.210/serde/ser/trait.Serialize.html#tymethod.serialize\" class=\"fn\">serialize</a>&lt;__S&gt;(&amp;self, __serializer: __S) -&gt; <a class=\"enum\" href=\"https://doc.rust-lang.org/1.82.0/core/result/enum.Result.html\" title=\"enum core::result::Result\">Result</a>&lt;__S::<a class=\"associatedtype\" href=\"https://docs.rs/serde/1.0.210/serde/ser/trait.Serializer.html#associatedtype.Ok\" title=\"type serde::ser::Serializer::Ok\">Ok</a>, __S::<a class=\"associatedtype\" href=\"https://docs.rs/serde/1.0.210/serde/ser/trait.Serializer.html#associatedtype.Error\" title=\"type serde::ser::Serializer::Error\">Error</a>&gt;<div class=\"where\">where\n    __S: <a class=\"trait\" href=\"https://docs.rs/serde/1.0.210/serde/ser/trait.Serializer.html\" title=\"trait serde::ser::Serializer\">Serializer</a>,</div></h4></section></summary><div class='docblock'>Serialize this value into the given Serde serializer. <a href=\"https://docs.rs/serde/1.0.210/serde/ser/trait.Serialize.html#tymethod.serialize\">Read more</a></div></details></div></details>","Serialize","iota_core::authority::authority_store_types::StoreMoveObject"],["<section id=\"impl-Eq-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-Eq-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/cmp/trait.Eq.html\" title=\"trait core::cmp::Eq\">Eq</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section>","Eq","iota_core::authority::authority_store_types::StoreMoveObject"],["<section id=\"impl-StructuralPartialEq-for-StoreMoveObjectV1\" class=\"impl\"><a class=\"src rightside\" href=\"src/iota_core/authority/authority_store_types.rs.html#172\">source</a><a href=\"#impl-StructuralPartialEq-for-StoreMoveObjectV1\" class=\"anchor\">§</a><h3 class=\"code-header\">impl <a class=\"trait\" href=\"https://doc.rust-lang.org/1.82.0/core/marker/trait.StructuralPartialEq.html\" title=\"trait core::marker::StructuralPartialEq\">StructuralPartialEq</a> for <a class=\"struct\" href=\"iota_core/authority/authority_store_types/struct.StoreMoveObjectV1.html\" title=\"struct iota_core::authority::authority_store_types::StoreMoveObjectV1\">StoreMoveObjectV1</a></h3></section>","StructuralPartialEq","iota_core::authority::authority_store_types::StoreMoveObject"]]]]);
    if (window.register_type_impls) {
        window.register_type_impls(type_impls);
    } else {
        window.pending_type_impls = type_impls;
    }
})()
//{"start":55,"fragment_lengths":[15703]}