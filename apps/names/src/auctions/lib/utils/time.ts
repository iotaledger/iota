// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AuctionMetadata } from '../types/metadata';

export function getTimeRemaining(auction: AuctionMetadata | null): number {
    if (!auction) return 0;

    const now = Date.now();
    return Math.max(0, auction.endTimestamp.getTime() - now);
}

export function formatTimeRemaining(milliseconds: number): string {
    if (milliseconds <= 0) return 'Finished';

    const seconds = Math.floor(milliseconds / 1000);
    const minutes = Math.floor(seconds / 60);
    const hours = Math.floor(minutes / 60);
    const days = Math.floor(hours / 24);

    if (days > 0) return `${days}d ${hours % 24}h`;
    if (hours > 0) return `${hours}h ${minutes % 60}m`;
    if (minutes > 0) return `${minutes}m ${seconds % 60}s`;
    return `${seconds}s`;
}
