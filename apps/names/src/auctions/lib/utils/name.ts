// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/**
 * Helper function to create a name object from a name string
 * Note: The labels are stored in reverse order according to Name struct
 * https://github.com/iotaledger/iota/blob/797355f33d982eb90b59542c4bceb0b1c6f8145f/crates/iota-names/src/name.rs#L19
 * @param name - Name like "rust.iota"
 * @returns Name object with labels in reverse order
 */
export function createNameObject(name: string): { labels: string[] } {
    const labels = name.split('.');
    return { labels: labels.reverse() };
}
