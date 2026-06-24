// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::menu;

use iota::dynamic_field;

/// A shared restaurant menu: a fixed list of regular dishes, plus any number of
/// special dishes stored as dynamic fields.
public struct Menu has key {
    id: UID,
    /// The regular dishes, always on offer.
    dishes: vector<Dish>,
}

/// A dish on the menu.
public struct Dish has store {
    name: vector<u8>,
    /// Price in the smallest currency unit.
    price: u64,
    vegetarian: bool,
    vegan: bool,
}

/// Dynamic-field key for a special dish.
public struct SpecialKey has copy, drop, store {
    name: vector<u8>,
}

/// Create and share a new menu with no dishes yet.
public fun create(ctx: &mut TxContext) {
    transfer::share_object(Menu { id: object::new(ctx), dishes: vector[] });
}

/// Add a dish to the regular menu.
///
/// This mutates the menu, so it is a regular function, not a view.
public fun add_dish(menu: &mut Menu, name: vector<u8>, price: u64, vegetarian: bool, vegan: bool) {
    menu.dishes.push_back(Dish { name, price, vegetarian, vegan });
}

/// Add a special dish as a dynamic field.
///
/// This mutates the menu, so it is a regular function, not a view.
public fun add_special(menu: &mut Menu, name: vector<u8>, price: u64, vegetarian: bool, vegan: bool) {
    dynamic_field::add(&mut menu.id, SpecialKey { name }, Dish { name, price, vegetarian, vegan });
}

/// Returns an immutable reference to the whole regular menu.
///
/// A view may return an immutable reference into an object it received by
/// reference; only mutable references are disallowed.
#[view]
public fun dishes(menu: &Menu): &vector<Dish> {
    &menu.dishes
}

/// Returns an immutable reference to the regular dish at `index`.
#[view]
public fun dish(menu: &Menu, index: u64): &Dish {
    &menu.dishes[index]
}

/// Returns an immutable reference to a special dish stored as a dynamic field.
#[view]
public fun special(menu: &Menu, name: vector<u8>): &Dish {
    dynamic_field::borrow(&menu.id, SpecialKey { name })
}

/// Returns the regular dish's price together with a reference to the dish.
///
/// A view may return a tuple, and that tuple may contain immutable references.
#[view]
public fun dish_with_price(menu: &Menu, index: u64): (u64, &Dish) {
    let dish = &menu.dishes[index];
    (dish.price, dish)
}

/// Reads the price held by a dish through an immutable reference.
#[view]
public fun price(dish: &Dish): u64 {
    dish.price
}
