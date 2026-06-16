// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module view_functions::leaderboard {
    /// A shared scoreboard tracking player scores.
    public struct Leaderboard has key {
        id: UID,
        entries: vector<Entry>,
    }

    public struct Entry has store, copy, drop {
        player: address,
        score: u64,
    }

    /// Create and share a new, empty leaderboard.
    public fun create(ctx: &mut TxContext) {
        transfer::share_object(Leaderboard {
            id: object::new(ctx),
            entries: vector[],
        });
    }

    /// Record a score for the transaction sender.
    ///
    /// This mutates the leaderboard, so it is a regular function, not a view.
    public fun submit_score(board: &mut Leaderboard, score: u64, ctx: &TxContext) {
        board.entries.push_back(Entry { player: ctx.sender(), score });
    }

    /// Returns the number of recorded scores.
    #[view]
    public fun total_entries(board: &Leaderboard): u64 {
        board.entries.length()
    }

    /// Returns the score recorded for `player`, or `0` if the player has none.
    #[view]
    public fun score_of(board: &Leaderboard, player: address): u64 {
        let mut i = 0;
        let n = board.entries.length();
        while (i < n) {
            let entry = &board.entries[i];
            if (entry.player == player) {
                return entry.score
            };
            i = i + 1;
        };
        0
    }

    /// Returns every recorded score, in submission order.
    #[view]
    public fun all_scores(board: &Leaderboard): vector<u64> {
        let mut scores = vector[];
        let mut i = 0;
        let n = board.entries.length();
        while (i < n) {
            scores.push_back(board.entries[i].score);
            i = i + 1;
        };
        scores
    }

    /// Returns whether `score` clears the high-score threshold.
    ///
    /// A pure view: it reads no on-chain state and only takes values by copy.
    #[view]
    public fun is_high_score(score: u64): bool {
        score >= 1000
    }
}
