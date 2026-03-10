// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function denormalizeName(name: string) {
    return name.toLowerCase().replace(/\.iota$/i, '');
}
