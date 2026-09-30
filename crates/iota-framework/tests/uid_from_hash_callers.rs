// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

//! Pins the set of `object::new_uid_from_hash` call sites in the framework.
//!
//! `new_uid_from_hash` mints a `UID` at a caller-chosen address, so it is the
//! only way to create an object whose id is not freshly derived. The account
//! design rests on the invariant that the sole caller deriving that address
//! from a *signature-derivable* value is `claim::claim_address` (reached only
//! through the `ClaimAccount` transaction kind): every other caller must
//! derive the address by hashing (`dynamic_field::hash_type_and_key`,
//! `derived_object::derive_address`), which no keypair can produce.
//!
//! A new call site can silently reopen that hole, so this test fails on any
//! change to the caller set. When it fails, review the new caller against the
//! invariant above before updating the expected list.

use std::{collections::BTreeMap, fs, path::Path};

#[test]
fn new_uid_from_hash_caller_set_is_pinned() {
    let sources = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("packages")
        .join("iota-framework")
        .join("sources");

    let mut call_sites = BTreeMap::new();
    collect_call_sites(&sources, &sources, &mut call_sites);

    // File (relative to the framework sources) -> number of call sites,
    // excluding the definition in object.move.
    let expected: BTreeMap<String, usize> = [
        ("account_abstraction/claim.move", 1),
        ("dynamic_field.move", 1),
        ("package_metadata/module_metadata.move", 1),
        ("package_metadata/package_metadata.move", 2),
    ]
    .into_iter()
    .map(|(file, count)| (file.to_string(), count))
    .collect();

    assert_eq!(
        call_sites, expected,
        "the `object::new_uid_from_hash` caller set changed; review every \
         new or moved call site against the invariant documented at the top \
         of this test before updating the expected list"
    );
}

fn collect_call_sites(root: &Path, dir: &Path, call_sites: &mut BTreeMap<String, usize>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            collect_call_sites(root, &path, call_sites);
            continue;
        }
        if path.extension().is_none_or(|extension| extension != "move") {
            continue;
        }
        let relative = path
            .strip_prefix(root)
            .unwrap()
            .to_str()
            .unwrap()
            .replace('\\', "/");
        let count = fs::read_to_string(&path)
            .unwrap()
            .lines()
            .filter(|line| {
                // The definition in object.move is not a call site.
                line.contains("new_uid_from_hash") && !line.contains("fun new_uid_from_hash")
            })
            .count();
        if count > 0 {
            call_sites.insert(relative, count);
        }
    }
}
