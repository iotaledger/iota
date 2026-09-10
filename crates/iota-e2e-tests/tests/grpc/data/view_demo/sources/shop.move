// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// A small merch shop, used to exercise `#[view]` return shapes: primitives,
/// references, vectors, options, tuples, and struct fields.
module view_demo::shop {
    use std::string::String;

    use iota::clock::Clock;

    public struct Shop has key {
        id: UID,
        name: String,
        sales: vector<u64>,
        owner: address,
    }

    public struct ShopSummary has copy, drop {
        total_revenue: u64,
        sale_count: u64,
        owner: address,
    }

    fun init(ctx: &mut TxContext) {
        transfer::share_object(Shop {
            id: object::new(ctx),
            name: std::string::utf8(b"IOTA Merch Store"),
            sales: vector[1000, 2500, 500],
            owner: ctx.sender(),
        });
    }

    /// Pure arguments, pure return: no `Shop` needed.
    #[view]
    public fun discounted_price(price: u64, pct: u64): u64 {
        price - price * pct / 100
    }

    #[view]
    public fun total_revenue(shop: &Shop): u64 {
        let mut total = 0;
        let mut i = 0;
        let len = shop.sales.length();
        while (i < len) {
            total = total + shop.sales[i];
            i = i + 1;
        };
        total
    }

    /// Reference return: renders the same as an owned `String`.
    #[view]
    public fun name(shop: &Shop): &String {
        &shop.name
    }

    #[view]
    public fun sales(shop: &Shop): vector<u64> {
        shop.sales
    }

    #[view]
    public fun sale_at(shop: &Shop, i: u64): Option<u64> {
        if (i < shop.sales.length()) {
            option::some(shop.sales[i])
        } else {
            option::none()
        }
    }

    /// Multiple return values.
    #[view]
    public fun stats(shop: &Shop): (u64, u64, address) {
        (total_revenue(shop), shop.sales.length(), shop.owner)
    }

    /// Struct return: rendered as a JSON object.
    #[view]
    public fun summary(shop: &Shop): ShopSummary {
        ShopSummary {
            total_revenue: total_revenue(shop),
            sale_count: shop.sales.length(),
            owner: shop.owner,
        }
    }

    #[view]
    public fun is_owner(shop: &Shop, who: address): bool {
        shop.owner == who
    }

    #[view]
    public fun open_for_ms(_shop: &Shop, clock: &Clock): u64 {
        clock.timestamp_ms()
    }

    /// Not `#[view]`: mutates `Shop`, used to check that view calls to
    /// non-view functions are rejected.
    public fun record_sale(shop: &mut Shop, amount: u64) {
        shop.sales.push_back(amount);
    }
}
