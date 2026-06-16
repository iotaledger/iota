// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::registry {
    use iota::dynamic_field;

    /// A shared registry with one primary setting and any number of named extra
    /// settings stored as dynamic fields.
    public struct Registry has key {
        id: UID,
        primary: Setting,
    }

    public struct Setting has store {
        value: u64,
    }

    public struct SettingKey has copy, drop, store {
        name: vector<u8>,
    }

    /// Create and share a registry with its primary setting.
    public fun create(primary_value: u64, ctx: &mut TxContext) {
        transfer::share_object(Registry {
            id: object::new(ctx),
            primary: Setting { value: primary_value },
        });
    }

    /// Add a named setting as a dynamic field.
    ///
    /// This mutates the registry, so it is a regular function, not a view.
    public fun add_setting(registry: &mut Registry, name: vector<u8>, value: u64) {
        dynamic_field::add(&mut registry.id, SettingKey { name }, Setting { value });
    }

    /// Returns an immutable reference to the primary setting.
    ///
    /// A view may return an immutable reference into an object it received by
    /// reference; only mutable references are disallowed.
    #[view]
    public fun primary(registry: &Registry): &Setting {
        &registry.primary
    }

    /// Returns an immutable reference to a named setting stored as a dynamic field.
    #[view]
    public fun setting(registry: &Registry, name: vector<u8>): &Setting {
        dynamic_field::borrow(&registry.id, SettingKey { name })
    }

    /// Returns the primary setting's value together with a reference to it.
    ///
    /// A view may return a tuple, and that tuple may contain immutable references.
    #[view]
    public fun primary_with_value(registry: &Registry): (u64, &Setting) {
        (registry.primary.value, &registry.primary)
    }

    /// Reads the value held by a setting through an immutable reference.
    #[view]
    public fun setting_value(setting: &Setting): u64 {
        setting.value
    }
}
