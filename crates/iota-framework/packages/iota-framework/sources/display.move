// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Defines a `Display` struct which defines the way an Object
/// should be displayed. The intention is to keep data as independent
/// from its display as possible, protecting the development process
/// and keeping it separate from the ecosystem agreements.
///
/// Each of the fields of the `Display` object should allow for pattern
/// substitution and filling-in the pieces using the data from the object `T`.
///
/// More entry functions might be added in the future depending on the use cases.
module iota::display {
    use std::string::String;

    use iota::event;
    use iota::package::Publisher;
    use iota::system_admin_cap::IotaSystemAdminCap;
    use iota::vec_map::{Self, VecMap};

    /// For when `T` does not belong to the package `Publisher`.
    const ENotOwner: u64 = 0;

    /// For when vectors passed into one of the multiple insert functions
    /// don't match in their lengths.
    const EVecLengthMismatch: u64 = 1;

    /// The `Display<T>` object. Defines the way a `T` instance should be
    /// displayed. `Display` object can only be created and modified with
    /// ether a `Publisher` cap or `IotaSystemAdminCap`, making sure that
    /// the rules are set by the owner of the type or the system object.
    ///
    /// Each of the display properties should support patterns outside
    /// of the system, making it simpler to customize `Display` based
    /// on the property values of an Object.
    /// ```
    /// // Example of a display object
    /// Display<0x107A::nft::Nft> {
    ///  fields:
    ///    <name, "IOTEST Nft">
    ///    <link, "https://iotestnft.com/nft/{ id }">
    ///    <image, "https://api.iotestnft.com/nft/{ id }/svg">
    ///    <description, "One of many Iotest Nfts">
    /// }
    /// ```
    ///
    /// Uses only `String` type due to external-facing nature of the object,
    /// the property names have a priority over their types.
    public struct Display<phantom T: key> has key, store {
        id: UID,
        /// Contains fields for display. Currently supported
        /// fields are: name, link, image and description.
        fields: VecMap<String, String>,
        /// Version that can only be updated manually by the `Publisher`.
        version: u16
    }

    /// Event: emitted when a new `Display` object has been created for type `T`.
    /// Type signature of the event corresponds to the type while id serves for
    /// the discovery.
    ///
    /// Since Iota RPC supports querying events by type, finding a `Display` for the `T`
    /// would be as simple as looking for the first event with `Display<T>`.
    public struct DisplayCreated<phantom T: key> has copy, drop {
        id: ID
    }

    /// Version of `Display` got updated.
    public struct VersionUpdated<phantom T: key> has copy, drop {
        id: ID,
        version: u16,
        fields: VecMap<String, String>,
    }

    // === Initializer Methods ===

    /// Create an empty `Display` object with `Publisher`.
    public fun new<T: key>(pub: &Publisher, ctx: &mut TxContext): Display<T> {
        assert!(is_authorized<T>(pub), ENotOwner);
        create_internal(ctx)
    }

    /// Create a new `Display<T>` object with a set of fields using `Publisher`.
    public fun new_with_fields<T: key>(
        pub: &Publisher, fields: vector<String>, values: vector<String>, ctx: &mut TxContext
    ): Display<T> {
        let mut display = new<T>(pub, ctx);
        add_multiple(&mut display, fields, values);
        display
    }

    /// Create an empty `Display` object with `IotaSystemAdminCap`.
    public fun system_new<T: key>(_: &IotaSystemAdminCap, ctx: &mut TxContext): Display<T> {
        create_internal(ctx)
    }

    /// Create a new Display<T> object with a set of fields using `IotaSystemAdminCap`.
    public fun system_new_with_fields<T: key>(
        cap: &IotaSystemAdminCap, fields: vector<String>, values: vector<String>, ctx: &mut TxContext
    ): Display<T> {
        let mut display = system_new<T>(cap, ctx);
        add_multiple(&mut display, fields, values);
        display
    }

    // === Entry functions: Create ===

    #[allow(lint(self_transfer))]
    /// Create a new empty `Display<T>` object and keep it.
    entry public fun create_and_keep<T: key>(pub: &Publisher, ctx: &mut TxContext) {
        transfer::public_transfer(new<T>(pub, ctx), ctx.sender())
    }

    /// Manually bump the version and emit an event with the updated version's contents.
    entry public fun update_version<T: key>(
        display: &mut Display<T>
    ) {
        display.version = display.version + 1;
        event::emit(VersionUpdated<T> {
            version: display.version,
            fields: *&display.fields,
            id: display.id.to_inner(),
        })
    }

    // === Entry functions: Add/Modify fields ===

    /// Sets a custom `name` field with the `value`.
    entry public fun add<T: key>(self: &mut Display<T>, name: String, value: String) {
        self.add_internal(name, value)
    }

    /// Sets multiple `fields` with `values`.
    entry public fun add_multiple<T: key>(
        self: &mut Display<T>, fields: vector<String>, values: vector<String>
    ) {
        let len = fields.length();
        assert!(len == values.length(), EVecLengthMismatch);

        let mut i = 0;
        while (i < len) {
            self.add_internal(fields[i], values[i]);
            i = i + 1;
        };
    }

    /// Change the value of the field.
    /// TODO (long run): version changes;
    entry public fun edit<T: key>(self: &mut Display<T>, name: String, value: String) {
        let (_, _) = self.fields.remove(&name);
        self.add_internal(name, value)
    }

    /// Remove the key from the `Display`.
    entry public fun remove<T: key>(self: &mut Display<T>, name: String) {
        self.fields.remove(&name);
    }

    // === Access fields ===

    /// Authorization check; can be performed externally to implement protection rules for `Display`.
    public fun is_authorized<T: key>(pub: &Publisher): bool {
        pub.from_package<T>()
    }

    /// Read the `id` field.
    public fun id<T: key>(d: &Display<T>): &ID {
        d.id.uid_as_inner()
    }

    /// Read the `version` field.
    public fun version<T: key>(d: &Display<T>): u16 {
        d.version
    }

    /// Read the `fields` field.
    public fun fields<T: key>(d: &Display<T>): &VecMap<String, String> {
        &d.fields
    }

    // === Private functions ===

    /// Internal function to create a new `Display<T>`.
    fun create_internal<T: key>(ctx: &mut TxContext): Display<T> {
        let uid = object::new(ctx);

        event::emit(DisplayCreated<T> {
            id: uid.to_inner()
        });

        Display {
            id: uid,
            fields: vec_map::empty(),
            version: 0,
        }
    }

    /// Private method for inserting fields without security checks.
    fun add_internal<T: key>(display: &mut Display<T>, name: String, value: String) {
        display.fields.insert(name, value)
    }
}

#[test_only]
module iota::display_tests {
    use std::string::String;

    use iota::display;
    use iota::package;
    use iota::system_admin_cap;

    use iota::test_scenario as test;
    use iota::test_utils;

    #[allow(unused_field)]
    /// An example object.
    /// Purely for visibility.
    public struct IotestNft has key {
        id: UID,
        name: String
    }

    /// Test witness type to create a Publisher object.
    public struct IOTESTNFT has drop {}

    #[test]
    fun nft_test_init() {
        let mut test = test::begin(@0x2);
        let pub = package::test_claim(IOTESTNFT {}, test.ctx());

        // create a new display object
        let mut display = display::new<IotestNft>(&pub, test.ctx());

        display.add(b"name".to_string(), b"IOTEST Nft {name}".to_string());
        display.add(b"link".to_string(), b"https://iotestnft.com/nft/{id}".to_string());
        display.add(b"image".to_string(), b"https://api.iotestnft.com/nft/{id}/svg".to_string());
        display.add(b"description".to_string(), b"One of many Iotest Nfts".to_string());

        pub.burn_publisher();
        transfer::public_transfer(display, @0x2);
        test.end();
    }

    #[test]
    fun nft_test_sys_init() {
        let mut test = test::begin(@0x2);
        let cap = system_admin_cap::new_system_admin_cap_for_testing();

        // create a new display object
        let mut display = display::system_new<IotestNft>(&cap, test.ctx());

        display.add(b"name".to_string(), b"IOTEST Nft {name}".to_string());
        display.add(b"link".to_string(), b"https://iotestnft.com/nft/{id}".to_string());
        display.add(b"image".to_string(), b"https://api.iotestnft.com/nft/{id}/svg".to_string());
        display.add(b"description".to_string(), b"One of many Iotest Nfts".to_string());

        transfer::public_transfer(display, @0x2);

        test_utils::destroy(cap);
        test.end();
    }
}
