// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Declare the module with package name and module name
module first_package::my_module{

// Sword struct represents a magical sword object
// - 'key' ability allows it to be stored as a top-level object
// - 'store' ability allows it to be stored inside other structs
public struct Sword has key, store {
    id: UID, // Unique identifier for the sword object
    magic: u64, // Magic power level of the sword
    strength: u64, // strength of the sword
}

public struct Forge has key {
    id: UID,
    swords_created: u64,
}

/// Module initializer to be executed when this module is published
fun init(ctx: &mut TxContext) {
    let admin = Forge {
        id: object::new(ctx),
        swords_created: 0,
    };

    // transfer the forge object to the module/package publisher
    transfer::transfer(admin, tx_context::sender(ctx));
}

// === Accessors ===
// These provide read-only access to struct fields

// Returns the magic power of a sword
public fun magic(self: &Sword): u64 {
    self.magic
}

// Returns the physical strength of a sword
public fun strength(self: &Sword): u64 {
    self.strength
}

// Returns how many swords a forge has created
public fun swords_created(self: &Forge): u64 {
    self.swords_created
}

/// Constructor for creating swords
/// Creates a new sword and increments the forge's creation counter
public fun new_sword(forge: &mut Forge, magic: u64, strength: u64, ctx: &mut TxContext): Sword {
    forge.swords_created = forge.swords_created + 1;
    Sword {
        id: object::new(ctx),
        magic: magic,
        strength: strength,
    }
}

// Config struct for storing configuration data
public struct Config has key {
    id: UID,
    value: u64,
}

// === Utility Functions ===

/// Creates and immediately transfers a sword to a recipient
public fun create_sword(magic: u64, strength: u64, recipient: address, ctx: &mut TxContext) {
    // Create new sword
    let sword = Sword {
        id: object::new(ctx),
        magic: magic,
        strength: strength,
    };
    // Transfer to recipient
    transfer::transfer(sword, recipient);
}

/// Transfers an existing sword to a new owner
public fun sword_transfer(sword: Sword, recipient: address, _ctx: &mut TxContext) {
    transfer::public_transfer(sword, recipient);
}

/// Creates a new configuration object
public fun create_config(value: u64, ctx: &mut TxContext): Config {
    Config {
        id: object::new(ctx),
        value: value,
    }
}

// === Tests ===
#[test_only]
use iota::test_scenario as ts; // Test scenario utilities
#[test_only]
use iota::test_utils;  // Additional test helpers

 // Test addresses
#[test_only]
const ADMIN: address = @0xAD;
#[test_only]
const ALICE: address = @0xA;
#[test_only]
const BOB: address = @0xB;

#[test]
public fun test_sword() {
    // Create a dummy TxContext for testing.
    let mut ctx = tx_context::dummy();

    // Create a sword.
    let sword = Sword {
        id: object::new(&mut ctx),
        magic: 42,
        strength: 7,
    };

    // Check if accessor functions return correct values.
    assert!(magic(&sword) == 42 && strength(&sword) == 7, 1);

    // Create a dummy address and transfer the sword.
    let dummy_address = @0xCAFE;
    transfer::transfer(sword, dummy_address);
}

#[test]
public fun test_module_init() {
    let mut ts = ts::begin(ADMIN);

    // first transaction to emulate module initialization.
    {
        ts::next_tx(&mut ts, ADMIN);
        init(ts::ctx(&mut ts));
    };

    // second transaction to check if the forge has been created
    // and has initial value of zero swords created
    {
        ts::next_tx(&mut ts, ADMIN);

        // extract the Forge object
        let forge: Forge = ts::take_from_sender(&ts);

        // verify number of created swords
        assert!(swords_created(&forge) == 0, 1);

        // return the Forge object to the object pool
        ts::return_to_sender(&ts, forge);
    };

    ts::end(ts);
}

#[test]
fun test_sword_transactions() {
    let mut ts = ts::begin(ADMIN);

    // first transaction to emulate module initialization
    {
        ts::next_tx(&mut ts, ADMIN);
        init(ts::ctx(&mut ts));
    };

    // second transaction executed by admin to create the sword
    {
        ts::next_tx(&mut ts, ADMIN);
        let mut forge: Forge = ts::take_from_sender(&ts);
        // create the sword and transfer it to the initial owner
        let sword = new_sword(&mut forge, 42, 7, ts::ctx(&mut ts));
        transfer::public_transfer(sword, ALICE);
        ts::return_to_sender(&ts, forge);
    };

    // third transaction executed by the initial sword owner
    {
        ts::next_tx(&mut ts, ALICE);
        // extract the sword owned by the initial owner
        let sword: Sword = ts::take_from_sender(&ts);
        // transfer the sword to the final owner
        transfer::public_transfer(sword, BOB);
    };

    // fourth transaction executed by the final sword owner
    {
        ts::next_tx(&mut ts, BOB);
        // extract the sword owned by the final owner
        let sword: Sword = ts::take_from_sender(&ts);
        // verify that the sword has expected properties
        assert!(magic(&sword) == 42 && strength(&sword) == 7, 1);
        // return the sword to the object pool (it cannot be dropped)
        ts::return_to_sender(&ts, sword)
    };

    ts::end(ts);
}

#[test]
fun test_assert_utils() {
    // Test equality assertions
    test_utils::assert_eq(10, 10);

    // Test vector equality
    let v1 = vector[1, 2, 3];
    let v2 = vector[3, 2, 1];
    test_utils::assert_same_elems(v1, v2);

    // Test object destruction
    let sword = Sword {
        id: object::new(&mut tx_context::dummy()),
        magic: 42,
        strength: 7,
    };
    test_utils::destroy(sword);
}

#[test]
fun test_scenario_advanced() {
    let mut ts = ts::begin(ADMIN);

    // 1. Initialize module
    {
        init(ts::ctx(&mut ts));
        let forge = Forge {
            id: object::new(ts::ctx(&mut ts)),
            swords_created: 0,
        };
        transfer::transfer(forge, ADMIN);
    };

    // 2. Create sword
    let effects = ts::next_tx(&mut ts, ADMIN);
    {
        let mut forge = ts::take_from_sender<Forge>(&ts);
        let sword = Sword {
            id: object::new(ts::ctx(&mut ts)),
            magic: 42,
            strength: 7,
        };
        forge.swords_created = forge.swords_created + 1;
        transfer::transfer(sword, ALICE);
        ts::return_to_sender(&ts, forge);

        // Test effects inspection
        assert!(ts::created(&effects).length() > 0, 1);
    };

    // 3. Transfer sword
    ts::next_tx(&mut ts, ALICE);
    {
        let sword = ts::take_from_sender<Sword>(&ts);
        ts::return_to_sender(&ts, sword); // Return instead of transfer for demo
    };

    // 4. Test shared object
    ts::next_tx(&mut ts, ADMIN);
    {
        let config = create_config(500, ts::ctx(&mut ts));
        transfer::share_object(config);
    };

    // 5. Access shared object
    ts::next_tx(&mut ts, ALICE);
    {
        let config = ts::take_shared<Config>(&ts);
        assert!(config.value == 500, 1);
        ts::return_shared(config);
    };

    // 6. Test epoch advancement
    ts::later_epoch(&mut ts, 1000, BOB);
    {
        let ctx = ts::ctx(&mut ts);
        assert!(tx_context::epoch(ctx) > 0, 1);
    };

    ts::end(ts);
}

#[test]
fun test_receiving_tickets() {
    let mut ts = ts::begin(ADMIN);

    // 1. Create sword in admin's inventory
    {
        let sword = Sword {
            id: object::new(ts::ctx(&mut ts)),
            magic: 42,
            strength: 7,
        };
        transfer::transfer(sword, ADMIN);
    };

    // 2. Admin creates receiving ticket
    ts::next_tx(&mut ts, ADMIN);
    {
        let sword_id = ts::most_recent_id_for_sender<Sword>(&ts).destroy_some();
        let receiving = ts::receiving_ticket_by_id<Sword>(sword_id);

        // Normally you'd transfer this to another object
        ts::return_receiving_ticket(receiving);
    };

    ts::end(ts);
}

#[test]
fun test_immutable_objects() {
    let mut ts = ts::begin(ADMIN);

    // 1. Create and freeze sword
    {
        let sword = Sword {
            id: object::new(ts::ctx(&mut ts)),
            magic: 42,
            strength: 7,
        };
        transfer::freeze_object(sword);
    };

    // 2. Access immutable object
    ts::next_tx(&mut ts, ALICE);
    {
        let sword = ts::take_immutable<Sword>(&ts);
        assert!(sword.magic == 42, 1);
        ts::return_immutable(sword);
    };

    ts::end(ts);
}

#[test]
fun test_address_operations() {
    let mut ts = ts::begin(ADMIN);

    // 1. Create and transfer sword
    {
        let sword = Sword {
            id: object::new(ts::ctx(&mut ts)),
            magic: 42,
            strength: 7,
        };
        transfer::transfer(sword, ALICE);
    };

    // 2. Access from specific address
    ts::next_tx(&mut ts, BOB);
    {
        // Directly take from Alice's address
        let sword = ts::take_from_address<Sword>(&ts, ALICE);
        assert!(ts::was_taken_from_address(ALICE, object::id(&sword)), 1);

        // Verify IDs list contains our sword
        let ids = ts::ids_for_address<Sword>(ALICE);
        assert!(vector::length(&ids) == 1, 1);

        ts::return_to_address(ALICE, sword);
    };

    ts::end(ts);
}
}
