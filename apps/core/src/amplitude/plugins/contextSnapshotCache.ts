// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Context Snapshot Cache
 *
 * Solves the timing issue where analytics events are processed after UI changes.
 * By caching UI context during event capture phase, we preserve state (e.g., dialog title)
 * before handlers modify/remove DOM elements. Cached snapshots expire after 1s.
 */

/**
 * Snapshot of UI context at a specific point in time.
 */
export interface ContextSnapshot {
    dialogTitle: string | null;
    timestamp: number;
}

/**
 * Cache for storing UI context snapshots with automatic expiration.
 */
export class ContextSnapshotCache {
    private snapshot: ContextSnapshot | null = null;
    private readonly ttlMs: number = 1000;

    /** Store a new snapshot, overwriting any existing one. */
    set(snapshot: ContextSnapshot): void {
        this.snapshot = snapshot;
    }

    /** Get cached snapshot if it exists and hasn't expired. */
    get(): ContextSnapshot | null {
        if (!this.snapshot) {
            return null;
        }

        const age = Date.now() - this.snapshot.timestamp;
        if (age > this.ttlMs) {
            this.snapshot = null;
            return null;
        }

        return this.snapshot;
    }

    /** Clear the cached snapshot. */
    clear(): void {
        this.snapshot = null;
    }
}

/** Global cache instance shared across the plugin lifecycle. */
export const contextSnapshotCache = new ContextSnapshotCache();
