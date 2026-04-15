// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Module defining the `Intent` type used for domain-separated signing.
///
/// An intent is a compact struct that serves as the domain separator for a
/// message that a signature commits to. It consists of three parts:
///     1. `scope` — what the type of the message is
///     2. `version` — the intent version
///     3. `app_id` — what application the signature refers to
///
/// The serialization of an Intent is a 3-byte array where each field is
/// represented by a byte and it is prepended onto a message before it is signed.
module iota::intent;

// === Structs ===

/// A signing intent used for domain separation.
public struct Intent has copy, drop, store {
    scope: u8,
    version: u8,
    app_id: u8,
}

// === Constructor ===

/// Create a new `Intent` with the given scope, version, and app id.
public fun new(scope: u8, version: u8, app_id: u8): Intent {
    Intent { scope, version, app_id }
}

// === Accessors ===

/// Returns the scope byte of the intent.
public fun scope(self: &Intent): u8 { self.scope }

/// Returns the version byte of the intent.
public fun version(self: &Intent): u8 { self.version }

/// Returns the app id byte of the intent.
public fun app_id(self: &Intent): u8 { self.app_id }

// === Scope constants ===

/// Used for a user signature on transaction data.
public fun scope_transaction_data(): u8 { 0 }

/// Used for an authority signature on transaction effects.
public fun scope_transaction_effects(): u8 { 1 }

/// Used for an authority signature on a checkpoint summary.
public fun scope_checkpoint_summary(): u8 { 2 }

/// Used for a user signature on a personal message.
public fun scope_personal_message(): u8 { 3 }

/// Used for an authority signature on a user signed transaction.
public fun scope_sender_signed_transaction(): u8 { 4 }

/// Used as a signature representing an authority's proof of possession of its authority key.
public fun scope_proof_of_possession(): u8 { 5 }

/// Deprecated. Should not be reused.
public fun scope_bridge_event_deprecated(): u8 { 6 }

/// Used for consensus authority signature on block's digest.
public fun scope_consensus_block(): u8 { 7 }

/// Used for reporting peer addresses in discovery.
public fun scope_discovery_peers(): u8 { 8 }

/// Used for authority capabilities from non-committee authorities.
public fun scope_authority_capabilities(): u8 { 9 }

// === Version constants ===

/// Intent version 0.
public fun version_v0(): u8 { 0 }

// === App ID constants ===

/// The IOTA application.
public fun app_id_iota(): u8 { 0 }

/// The Consensus application.
public fun app_id_consensus(): u8 { 1 }

// === Convenience constructors ===

/// Returns the standard IOTA transaction intent (scope=0, version=0, app_id=0).
public fun iota_transaction(): Intent {
    Intent { scope: scope_transaction_data(), version: version_v0(), app_id: app_id_iota() }
}

/// Returns the standard IOTA personal message intent (scope=3, version=0, app_id=0).
public fun iota_personal_message(): Intent {
    Intent { scope: scope_personal_message(), version: version_v0(), app_id: app_id_iota() }
}

/// Returns the intent bytes as a 3-byte vector.
public fun to_bytes(self: &Intent): vector<u8> {
    vector[self.scope, self.version, self.app_id]
}
