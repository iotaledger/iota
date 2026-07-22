// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

#[defines_primitive(vector)]
/// A variable-sized container that can hold any type. Indexing is 0-based, and
/// vectors are growable. This module has many native functions.
module std::vector;

/// Allows calling `.to_string()` on a vector of `u8` to get a utf8 `String`.
public use fun std::string::utf8 as vector.to_string;

/// Allows calling `.try_to_string()` on a vector of `u8` to get a utf8 `String`.
/// This will return `None` if the vector is not valid utf8.
public use fun std::string::try_utf8 as vector.try_to_string;

/// Allows calling `.to_ascii_string()` on a vector of `u8` to get an `ascii::String`.
public use fun std::ascii::string as vector.to_ascii_string;

/// Allows calling `.try_to_ascii_string()` on a vector of `u8` to get an
/// `ascii::String`. This will return `None` if the vector is not valid ascii.
public use fun std::ascii::try_string as vector.try_to_ascii_string;

/// The index into the vector is out of bounds
const EINDEX_OUT_OF_BOUNDS: u64 = 0x20000;

#[bytecode_instruction]
/// Create an empty vector.
public native fun empty<Element>(): vector<Element>;

#[bytecode_instruction]
/// Return the length of the vector.
public native fun length<Element>(v: &vector<Element>): u64;

#[syntax(index)]
#[bytecode_instruction]
/// Acquire an immutable reference to the `i`th element of the vector `v`.
/// Aborts if `i` is out of bounds.
public native fun borrow<Element>(v: &vector<Element>, i: u64): &Element;

#[bytecode_instruction]
/// Add element `e` to the end of the vector `v`.
public native fun push_back<Element>(v: &mut vector<Element>, e: Element);

#[syntax(index)]
#[bytecode_instruction]
/// Return a mutable reference to the `i`th element in the vector `v`.
/// Aborts if `i` is out of bounds.
public native fun borrow_mut<Element>(v: &mut vector<Element>, i: u64): &mut Element;

#[bytecode_instruction]
/// Pop an element from the end of vector `v`.
/// Aborts if `v` is empty.
public native fun pop_back<Element>(v: &mut vector<Element>): Element;

#[bytecode_instruction]
/// Destroy the vector `v`.
/// Aborts if `v` is not empty.
public native fun destroy_empty<Element>(v: vector<Element>);

#[bytecode_instruction]
/// Swaps the elements at the `i`th and `j`th indices in the vector `v`.
/// Aborts if `i` or `j` is out of bounds.
public native fun swap<Element>(v: &mut vector<Element>, i: u64, j: u64);

/// Return a vector of size one containing element `e`.
public fun singleton<Element>(e: Element): vector<Element> {
    let mut v = empty();
    v.push_back(e);
    v
}

/// Reverses the order of the elements in the vector `v` in place.
public fun reverse<Element>(v: &mut vector<Element>) {
    let len = v.length();
    if (len == 0) return ();

    let mut front_index = 0;
    let mut back_index = len - 1;
    while (front_index < back_index) {
        v.swap(front_index, back_index);
        front_index = front_index + 1;
        back_index = back_index - 1;
    }
}

/// Pushes all of the elements of the `other` vector into the `lhs` vector.
public fun append<Element>(lhs: &mut vector<Element>, other: vector<Element>) {
    other.do!(|e| lhs.push_back(e));
}

/// Return `true` if the vector `v` has no elements and `false` otherwise.
public fun is_empty<Element>(v: &vector<Element>): bool {
    v.length() == 0
}

/// Return true if `e` is in the vector `v`.
/// Otherwise, returns false.
public fun contains<Element>(v: &vector<Element>, e: &Element): bool {
    let mut i = 0;
    let len = v.length();
    while (i < len) {
        if (&v[i] == e) return true;
        i = i + 1;
    };
    false
}

/// Return `(true, i)` if `e` is in the vector `v` at index `i`.
/// Otherwise, returns `(false, 0)`.
public fun index_of<Element>(v: &vector<Element>, e: &Element): (bool, u64) {
    let mut i = 0;
    let len = v.length();
    while (i < len) {
        if (&v[i] == e) return (true, i);
        i = i + 1;
    };
    (false, 0)
}

/// Remove the `i`th element of the vector `v`, shifting all subsequent elements.
/// This is O(n) and preserves ordering of elements in the vector.
/// Aborts if `i` is out of bounds.
public fun remove<Element>(v: &mut vector<Element>, mut i: u64): Element {
    let mut len = v.length();
    // i out of bounds; abort
    if (i >= len) abort EINDEX_OUT_OF_BOUNDS;

    len = len - 1;
    while (i < len) {
        v.swap(i, { i = i + 1; i });
    };
    v.pop_back()
}

/// Insert `e` at position `i` in the vector `v`.
/// If `i` is in bounds, this shifts the old `v[i]` and all subsequent elements to the right.
/// If `i == v.length()`, this adds `e` to the end of the vector.
/// This is O(n) and preserves ordering of elements in the vector.
/// Aborts if `i > v.length()`
public fun insert<Element>(v: &mut vector<Element>, e: Element, mut i: u64) {
    let len = v.length();
    // i too big abort
    if (i > len) abort EINDEX_OUT_OF_BOUNDS;

    v.push_back(e);
    while (i < len) {
        v.swap(i, len);
        i = i + 1
    }
}

/// Swap the `i`th element of the vector `v` with the last element and then pop the vector.
/// This is O(1), but does not preserve ordering of elements in the vector.
/// Aborts if `i` is out of bounds.
public fun swap_remove<Element>(v: &mut vector<Element>, i: u64): Element {
    assert!(v.length() != 0, EINDEX_OUT_OF_BOUNDS);
    let last_idx = v.length() - 1;
    v.swap(i, last_idx);
    v.pop_back()
}

// === Macros ===

/// Create a vector of length `n` by calling the function `f` on each index.
public macro fun tabulate<$T>($n: u64, $f: |u64| -> $T): vector<$T> {
    let mut v = vector[];
    let n = $n;
    n.do!(|i| v.push_back($f(i)));
    v
}

/// Destroy the vector `v` by calling `f` on each element and then destroying the vector.
/// Does not preserve the order of elements in the vector (starts from the end of the vector).
public macro fun destroy<$T, $R: drop>($v: vector<$T>, $f: |$T| -> $R) {
    let mut v = $v;
    v.length().do!(|_| $f(v.pop_back()));
    v.destroy_empty();
}

/// Destroy the vector `v` by calling `f` on each element and then destroying the vector.
/// Preserves the order of elements in the vector.
public macro fun do<$T, $R: drop>($v: vector<$T>, $f: |$T| -> $R) {
    let mut v = $v;
    v.reverse();
    v.length().do!(|_| $f(v.pop_back()));
    v.destroy_empty();
}

/// Perform an action `f` on each element of the vector `v`. The vector is not modified.
public macro fun do_ref<$T, $R: drop>($v: &vector<$T>, $f: |&$T| -> $R) {
    let v = $v;
    v.length().do!(|i| $f(&v[i]))
}

/// Perform an action `f` on each element of the vector `v`.
/// The function `f` takes a mutable reference to the element.
public macro fun do_mut<$T, $R: drop>($v: &mut vector<$T>, $f: |&mut $T| -> $R) {
    let v = $v;
    v.length().do!(|i| $f(&mut v[i]))
}

/// Map the vector `v` to a new vector by applying the function `f` to each element.
/// Preserves the order of elements in the vector, first is called first.
public macro fun map<$T, $U>($v: vector<$T>, $f: |$T| -> $U): vector<$U> {
    let v = $v;
    let mut r = vector[];
    v.do!(|e| r.push_back($f(e)));
    r
}

/// Map the vector `v` to a new vector by applying the function `f` to each element.
/// Preserves the order of elements in the vector, first is called first.
public macro fun map_ref<$T, $U>($v: &vector<$T>, $f: |&$T| -> $U): vector<$U> {
    let v = $v;
    let mut r = vector[];
    v.do_ref!(|e| r.push_back($f(e)));
    r
}

/// Filter the vector `v` by applying the function `f` to each element.
/// Return a new vector containing only the elements for which `f` returns `true`.
public macro fun filter<$T: drop>($v: vector<$T>, $f: |&$T| -> bool): vector<$T> {
    let v = $v;
    let mut r = vector[];
    v.do!(|e| if ($f(&e)) r.push_back(e));
    r
}

/// Split the vector `v` into two vectors by applying the function `f` to each element.
/// Return a tuple containing two vectors: the first containing the elements for which `f` returns `true`,
/// and the second containing the elements for which `f` returns `false`.
public macro fun partition<$T>($v: vector<$T>, $f: |&$T| -> bool): (vector<$T>, vector<$T>) {
    let v = $v;
    let mut r1 = vector[];
    let mut r2 = vector[];
    v.do!(|e| if ($f(&e)) r1.push_back(e) else r2.push_back(e));
    (r1, r2)
}

/// Finds the index of first element in the vector `v` that satisfies the predicate `f`.
/// Returns `some(index)` if such an element is found, otherwise `none()`.
public macro fun find_index<$T>($v: &vector<$T>, $f: |&$T| -> bool): Option<u64> {
    let v = $v;
    'find_index: {
        v.length().do!(|i| if ($f(&v[i])) return 'find_index option::some(i));
        option::none()
    }
}

/// Count how many elements in the vector `v` satisfy the predicate `f`.
public macro fun count<$T>($v: &vector<$T>, $f: |&$T| -> bool): u64 {
    let v = $v;
    let mut count = 0;
    v.do_ref!(|e| if ($f(e)) count = count + 1);
    count
}

/// Reduce the vector `v` to a single value by applying the function `f` to each element.
/// Similar to `fold_left` in Rust and `reduce` in Python and JavaScript.
public macro fun fold<$T, $Acc>($v: vector<$T>, $init: $Acc, $f: |$Acc, $T| -> $Acc): $Acc {
    let v = $v;
    let mut acc = $init;
    v.do!(|e| acc = $f(acc, e));
    acc
}

/// Concatenate the vectors of `v` into a single vector, keeping the order of the elements.
public fun flatten<T>(v: vector<vector<T>>): vector<T> {
    let mut r = vector[];
    v.do!(|u| r.append(u));
    r
}

/// Whether any element in the vector `v` satisfies the predicate `f`.
/// If the vector is empty, returns `false`.
public macro fun any<$T>($v: &vector<$T>, $f: |&$T| -> bool): bool {
    let v = $v;
    'any: {
        v.do_ref!(|e| if ($f(e)) return 'any true);
        false
    }
}

/// Whether all elements in the vector `v` satisfy the predicate `f`.
/// If the vector is empty, returns `true`.
public macro fun all<$T>($v: &vector<$T>, $f: |&$T| -> bool): bool {
    let v = $v;
    'all: {
        v.do_ref!(|e| if (!$f(e)) return 'all false);
        true
    }
}

/// Perform an action `f` on the `ix[i]`-th element of the vector `v`
/// for each `i` in `ix`. The vector `v` is not modified.
///
/// Pseudocode: for each `i` call `f(&v[ix[i]])`.
public macro fun take_do_ref<$T>($v: &vector<$T>, $ix: &vector<u64>, $f: |&$T|) {
    let v = $v;
    let ix = $ix;
    let v_len = v.length();
    ix.do_ref!(|i| {
        assert!(*i < v_len);
        $f(&v[*i]);
    });
}

/// Perform a mutating action `f` on the `ix[i]`-th element of the vector
/// `v` for each `i` in `ix`. The vector `v` can be modified.
///
/// Pseudocode: for each `i` call `f(&mut v[ix[i]])`.
public macro fun take_do_mut<$T>($v: &mut vector<$T>, $ix: &vector<u64>, $f: |&mut $T|) {
    let v = $v;
    let ix = $ix;
    let v_len = v.length();
    ix.do_ref!(|i| {
        assert!(*i < v_len);
        $f(&mut v[*i]);
    });
}

/// Perform an action `f` on the index `i`, value of `ix[i]` and
/// the `ix[i]`-th element of the vector `v` for each `i` in `ix`.
/// The vector `v` is not modified.
///
/// Pseudocode: for each `i` call `f(i, ix[i], &v[ix[i]])`.
public macro fun take_do_with_ix_ref<$T>($v: &vector<$T>, $ix: &vector<u64>, $f: |u64, u64, &$T|) {
    let v = $v;
    let ix = $ix;
    let v_len = v.length();
    ix.length().do!(|k| {
        let i = ix[k];
        assert!(i < v_len);
        $f(k, i, &v[i]);
    });
}

/// Perform a mutating action `f` on the index `i`, value of `ix[i]` and
/// the `ix[i]`-th element of the vector `v` for each `i` in `ix`.
/// The vector `v` can be modified.
///
/// Pseudocode: for each `i` call `f(i, ix[i], &mut v[ix[i]])`.
public macro fun take_do_with_ix_mut<$T>(
    $v: &mut vector<$T>,
    $ix: &vector<u64>,
    $f: |u64, u64, &mut $T|,
) {
    let v = $v;
    let ix = $ix;
    let v_len = v.length();
    ix.length().do!(|k| {
        let i = ix[k];
        assert!(i < v_len);
        $f(k, i, &mut v[i]);
    });
}

/// Find the smallest `i` such that `f(&v[ix[i]])` is true and return `some(ix[i])`.
/// Return `none` if `f` is false for all `i`.
/// Note: this is different from `find_index!($v, $f)` because `ix` serves as a filter.
public macro fun take_find_index<$T>(
    $v: &vector<$T>,
    $ix: &vector<u64>,
    $f: |&$T| -> bool,
): Option<u64> {
    'take_find_index: {
        take_do_with_ix_ref!($v, $ix, |_, i, x| {
            if ($f(x)) return 'take_find_index option::some(i);
        });
        option::none()
    }
}

/// Map the `ix[i]`-th element of the vector `v` with a function `f`
/// for each `i` in `ix` and collect the resulting values into a new vector.
/// The vector `v` is not modified.
///
/// Pseudocode: for each `i` return vector `u` such that `u[i] = f(&v[ix[i]])`.
public macro fun take_map_ref<$T, $U>(
    $v: &vector<$T>,
    $ix: &vector<u64>,
    $f: |&$T| -> $U,
): vector<$U> {
    let mut u = vector::empty<$U>();
    take_do_ref!($v, $ix, |x| u.push_back($f(x)));
    u
}

/// Take copies of every `ix[i]`-th element of the vector `v`
/// for each `i` in `ix` and collect them into a new vector.
/// The vector `v` is not modified.
///
/// Pseudocode: for each `i` return vector `u` such that `u[i] = v[ix[i]]`.
public macro fun take_collect<$T: copy>($v: &vector<$T>, $ix: &vector<u64>): vector<$T> {
    take_map_ref!($v, $ix, |x| *x)
}

/// Select the first `min(n,v.length())` largest values in `v` with respect to
/// comparator `less_than` and return the corresponding indices.
/// The returned values are not necessarily ordered.
public macro fun take_top_n<$T>(
    $v: &vector<$T>,
    $n: u64,
    $less_than: |&$T, &$T| -> bool,
): vector<u64> {
    let v = $v;
    let v_len = v.length();
    let n = $n;

    'take_top_n: { if (v_len <= n) {
            return 'take_top_n vector::tabulate!(v_len, |i| i)
        } else if (n == 0) {
            return 'take_top_n vector::empty<u64>()
        };  // 0 < n < v_len
        // unroll the first iteration
        // indices of top min(n,i) elements in descending order
        let mut ix = vector[0_u64]; let mut i = 1;  while (i < v_len) {
            let x = &v[i];
            let mut j = ix.length() - 1;
            if (i < n) {
                // i == ix.length() == j + 1
                // not enough elements, put x into ix anyway
                ix.push_back(i);
            };

            // compare the smallest of the top min(n,i) elements and the current one
            if ($less_than(&v[ix[j]], x)) {
                if (i < n) {
                    // index of x is already at pos j+1 in ix
                    // i == j + 1
                    ix.swap(i, j);
                } else {
                    // overwrite the smallest with the index of new larger value
                    *ix.borrow_mut(j) = i;
                };
                // x == v[ix[j]]
                while (j > 0 && $less_than(&v[ix[j-1]], x)) {
                    ix.swap(j, j - 1);
                    j = j - 1;
                };
            };

            i = i + 1;
        };  ix }
}

/// Folds every `ix[i]`-th element of the vector `v` for each `i` in `ix`
/// into an accumulator with initial value `init` by applying function `f`
/// and returns the resulting accumulator.
/// The vector `v` is not modified.
public macro fun take_fold_ref<$T, $Acc>(
    $v: &vector<$T>,
    $ix: &vector<u64>,
    $init: $Acc,
    $f: |$Acc, &$T| -> $Acc,
): $Acc {
    let mut acc = $init;
    take_do_ref!($v, $ix, |x| {
        acc = $f(acc, x);
    });
    acc
}

/// Destroys two vectors `v1` and `v2` by calling `f` to each pair of elements.
/// Aborts if the vectors are not of the same length.
/// The order of elements in the vectors is preserved.
public macro fun zip_do<$T1, $T2, $R: drop>(
    $v1: vector<$T1>,
    $v2: vector<$T2>,
    $f: |$T1, $T2| -> $R,
) {
    let v1 = $v1;
    let mut v2 = $v2;
    v2.reverse();
    let len = v1.length();
    assert!(len == v2.length());
    v1.do!(|el1| $f(el1, v2.pop_back()));
}

/// Destroys two vectors `v1` and `v2` by calling `f` to each pair of elements.
/// Aborts if the vectors are not of the same length.
/// Starts from the end of the vectors.
public macro fun zip_do_reverse<$T1, $T2, $R: drop>(
    $v1: vector<$T1>,
    $v2: vector<$T2>,
    $f: |$T1, $T2| -> $R,
) {
    let v1 = $v1;
    let mut v2 = $v2;
    let len = v1.length();
    assert!(len == v2.length());
    v1.destroy!(|el1| $f(el1, v2.pop_back()));
    v2.destroy_empty();
}

/// Iterate through `v1` and `v2` and apply the function `f` to references of each pair of
/// elements. The vectors are not modified.
/// Aborts if the vectors are not of the same length.
/// The order of elements in the vectors is preserved.
public macro fun zip_do_ref<$T1, $T2, $R: drop>(
    $v1: &vector<$T1>,
    $v2: &vector<$T2>,
    $f: |&$T1, &$T2| -> $R,
) {
    let v1 = $v1;
    let v2 = $v2;
    let len = v1.length();
    assert!(len == v2.length());
    len.do!(|i| $f(&v1[i], &v2[i]));
}

/// Iterate through `v1` and `v2` and apply the function `f` to mutable references of each pair
/// of elements. The vectors may be modified.
/// Aborts if the vectors are not of the same length.
/// The order of elements in the vectors is preserved.
public macro fun zip_do_mut<$T1, $T2, $R: drop>(
    $v1: &mut vector<$T1>,
    $v2: &mut vector<$T2>,
    $f: |&mut $T1, &mut $T2| -> $R,
) {
    let v1 = $v1;
    let v2 = $v2;
    let len = v1.length();
    assert!(len == v2.length());
    len.do!(|i| $f(&mut v1[i], &mut v2[i]));
}

/// Destroys two vectors `v1` and `v2` by applying the function `f` to each pair of elements.
/// The returned values are collected into a new vector.
/// Aborts if the vectors are not of the same length.
/// The order of elements in the vectors is preserved.
public macro fun zip_map<$T1, $T2, $U>(
    $v1: vector<$T1>,
    $v2: vector<$T2>,
    $f: |$T1, $T2| -> $U,
): vector<$U> {
    let mut r = vector[];
    zip_do!($v1, $v2, |el1, el2| r.push_back($f(el1, el2)));
    r
}

/// Iterate through `v1` and `v2` and apply the function `f` to references of each pair of
/// elements. The returned values are collected into a new vector.
/// Aborts if the vectors are not of the same length.
/// The order of elements in the vectors is preserved.
public macro fun zip_map_ref<$T1, $T2, $U>(
    $v1: &vector<$T1>,
    $v2: &vector<$T2>,
    $f: |&$T1, &$T2| -> $U,
): vector<$U> {
    let mut r = vector[];
    zip_do_ref!($v1, $v2, |el1, el2| r.push_back($f(el1, el2)));
    r
}
