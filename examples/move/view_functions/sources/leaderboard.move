// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::leaderboard;

#[error(code = 0)]
const ENotAnAdmin: vector<u8> = b"Only admin allowed.";

/// A shared leaderboard, with entries ranked from highest score to lowest.
public struct Leaderboard has key {
    id: UID,
    admin: address,
    entries: vector<ScoreEntry>,
}

/// A single player's standing on the leaderboard.
public struct ScoreEntry has copy, drop, store {
    player: address,
    score: u64,
}

/// Create and share a new, empty leaderboard.
public fun create(ctx: &mut TxContext) {
    transfer::share_object(Leaderboard {
        id: object::new(ctx),
        admin: ctx.sender(),
        entries: vector[],
    });
}

/// Record a score for `player`, keeping the leaderboard ranked highest-first.
///
/// If the player already has a score it is replaced and re-ranked.
public fun submit_score(board: &mut Leaderboard, player: address, score: u64, ctx: &TxContext) {
    assert!(ctx.sender() == board.admin, ENotAnAdmin);

    // Drop any existing entry for this player so the new score is the only one.
    let mut i = 0;
    while (i < board.entries.length()) {
        if (board.entries[i].player == player) {
            board.entries.remove(i);
            break
        };
        i = i + 1;
    };

    // Insert ahead of the first entry with a strictly lower score, keeping the
    // vector sorted in descending order.
    let mut pos = 0;
    while (pos < board.entries.length() && board.entries[pos].score >= score) {
        pos = pos + 1;
    };
    board.entries.insert(ScoreEntry { player, score }, pos);
}

/// Returns the number of recorded scores.
#[view]
public fun total_entries(board: &Leaderboard): u64 {
    board.entries.length()
}

/// Returns the entry recorded for `player`, or `none` if they have no score.
#[view]
public fun score_of(board: &Leaderboard, player: &address): Option<ScoreEntry> {
    let mut i = 0;
    while (i < board.entries.length()) {
        let entry = &board.entries[i];
        if (&entry.player == player) return option::some(*entry);
        i = i + 1;
    };
    option::none()
}

/// Returns every recorded score, ranked from highest to lowest.
#[view]
public fun all_scores(board: &Leaderboard): vector<ScoreEntry> {
    board.entries
}

/// Returns the leading entry, or `none` if the leaderboard is empty.
///
/// Entries are kept ranked highest-first, so the leader is simply the first.
#[view]
public fun highest_score(board: &Leaderboard): Option<ScoreEntry> {
    if (board.entries.is_empty()) option::none() else option::some(board.entries[0])
}
