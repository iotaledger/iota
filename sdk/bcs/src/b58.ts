// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import bs58 from 'bs58';

export const toB58 = (buffer: Uint8Array) => bs58.encode(buffer);
export const fromB58 = (str: string) => bs58.decode(str);
