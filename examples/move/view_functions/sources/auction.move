// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::auction;

use iota::clock::Clock;
use std::string::String;
use iota::dynamic_field;

#[error(code = 0)]
const EAuctionEnded: vector<u8> = b"The auction has already ended.";
#[error(code = 1)]
const EBidTooLow: vector<u8> = b"The bid does not exceed the current highest bid.";

/// A shared English auction for a single lot.
public struct Auction has key {
    id: UID,
    /// Human-readable description of the lot on sale.
    lot: String,
    seller: address,
    /// Unix timestamp (ms) after which no further bids are accepted.
    ends_at: u64,
    /// The highest amount bid so far; zero until the first bid.
    highest_bid: u64,
    /// Every bid placed, in the order received.
    bids: vector<Bid>,
}

/// A single bid placed on the auction.
public struct Bid has copy, drop, store {
    bidder: address,
    /// Bid amount in the smallest currency unit.
    amount: u64,
}

/// Dynamic-field key for a bidder's running tally.
public struct BidderKey has copy, drop, store {
    bidder: address,
}

/// A bidder's activity, stored as a dynamic field keyed by their address.
public struct BidderTally has store {
    /// How many bids this bidder has placed.
    count: u64,
    /// This bidder's highest bid so far.
    highest: u64,
}

/// Create and share a new auction for `lot`, open until `ends_at`.
public fun create(lot: String, ends_at: u64, ctx: &mut TxContext) {
    transfer::share_object(Auction {
        id: object::new(ctx),
        lot,
        seller: ctx.sender(),
        ends_at,
        highest_bid: 0,
        bids: vector[],
    });
}

/// Place a bid on the auction.
///
/// Aborts if the auction has ended or if `amount` does not beat the current
/// highest bid. This mutates the auction, so it is a regular function, not a
/// view.
public fun place_bid(auction: &mut Auction, amount: u64, clock: &Clock, ctx: &TxContext) {
    assert!(clock.timestamp_ms() < auction.ends_at, EAuctionEnded);
    assert!(amount > auction.highest_bid, EBidTooLow);

    let bidder = ctx.sender();
    auction.highest_bid = amount;
    auction.bids.push_back(Bid { bidder, amount });

    let key = BidderKey { bidder };
    if (dynamic_field::exists_(&auction.id, key)) {
        let tally: &mut BidderTally = dynamic_field::borrow_mut(&mut auction.id, key);
        tally.count = tally.count + 1;
        tally.highest = amount;
    } else {
        dynamic_field::add(&mut auction.id, key, BidderTally { count: 1, highest: amount });
    }
}

/// Returns an immutable reference to the whole list of bids.
///
/// A view may return an immutable reference into an object it received by
/// reference; only mutable references are disallowed.
#[view]
public fun bids(auction: &Auction): &vector<Bid> {
    &auction.bids
}

/// Returns an immutable reference to the bid at `index`.
#[view]
public fun bid_at(auction: &Auction, index: u64): &Bid {
    &auction.bids[index]
}

/// Returns an immutable reference to a bidder's tally, stored as a dynamic field.
#[view]
public fun tally_of(auction: &Auction, bidder: address): &BidderTally {
    dynamic_field::borrow(&auction.id, BidderKey { bidder })
}

/// Returns the bid's amount together with a reference to the bid.
///
/// A view may return a tuple, and that tuple may contain immutable references.
#[view]
public fun bid_with_amount(auction: &Auction, index: u64): (u64, &Bid) {
    let bid = &auction.bids[index];
    (bid.amount, bid)
}

/// Reads the amount held by a bid through an immutable reference.
#[view]
public fun amount(bid: &Bid): u64 {
    bid.amount
}
