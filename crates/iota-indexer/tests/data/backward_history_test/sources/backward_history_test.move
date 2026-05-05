// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Test module exercising all object lifecycle events relevant to backward
/// history ingestion: create, mutate, wrap, unwrap, delete, and
/// unwrap-then-delete.
module backward_history_test::backward_history_test {

    /// A simple object that can be created, mutated, wrapped, and deleted.
    public struct Item has key, store {
        id: UID,
        value: u64,
    }

    /// A wrapper that holds an Item inside it (wrapping it).
    public struct Box has key, store {
        id: UID,
        item: Item,
    }

    /// Create a new Item and transfer it to the sender.
    public entry fun create(value: u64, ctx: &mut TxContext) {
        iota::transfer::public_transfer(
            Item { id: object::new(ctx), value },
            tx_context::sender(ctx),
        );
    }

    /// Mutate an existing Item by changing its value.
    public entry fun mutate(item: &mut Item, new_value: u64) {
        item.value = new_value;
    }

    /// Wrap an Item inside a Box. The Item disappears from the object store.
    public entry fun wrap(item: Item, ctx: &mut TxContext) {
        iota::transfer::public_transfer(
            Box { id: object::new(ctx), item },
            tx_context::sender(ctx),
        );
    }

    /// Unwrap an Item from a Box. The Box is destroyed and the Item reappears.
    public entry fun unwrap(box_obj: Box, ctx: &mut TxContext) {
        let Box { id, item } = box_obj;
        object::delete(id);
        iota::transfer::public_transfer(item, tx_context::sender(ctx));
    }

    /// Delete an Item permanently.
    public entry fun delete(item: Item) {
        let Item { id, value: _ } = item;
        object::delete(id);
    }

    /// Unwrap-then-delete: destroy both the Box and the Item inside it.
    /// The Item goes from wrapped → deleted without ever appearing in the
    /// object store, producing an "unwrapped_then_deleted" effect.
    public entry fun unwrap_and_delete(box_obj: Box) {
        let Box { id: box_id, item } = box_obj;
        object::delete(box_id);
        let Item { id: item_id, value: _ } = item;
        object::delete(item_id);
    }

    // ---------------------------------------------------------------
    // Dynamic object field lifecycle: add, remove, re-add
    // ---------------------------------------------------------------
    //
    // Used to exercise the case where a DOF Field-object id (derived from
    // (parent, name, type)) is created, deleted, and re-created with the same
    // id within the visible history window.

    /// A parent object that can have dynamic object fields attached.
    public struct Parent has key, store {
        id: UID,
    }

    /// A child object that can be stored as a dynamic object field value.
    public struct Child has key, store {
        id: UID,
        value: u64,
    }

    public entry fun create_parent(ctx: &mut TxContext) {
        iota::transfer::public_transfer(
            Parent { id: object::new(ctx) },
            tx_context::sender(ctx),
        );
    }

    public entry fun add_dof(parent: &mut Parent, name: u64, value: u64, ctx: &mut TxContext) {
        let child = Child { id: object::new(ctx), value };
        iota::dynamic_object_field::add(&mut parent.id, name, child);
    }

    public entry fun remove_dof(parent: &mut Parent, name: u64) {
        let child: Child = iota::dynamic_object_field::remove(&mut parent.id, name);
        let Child { id, value: _ } = child;
        object::delete(id);
    }
}
