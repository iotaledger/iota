// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { base58 } from '@scure/base';

export const toBase58 = (buffer: Uint8Array) => base58.encode(buffer);
export const fromBase58 = (str: string) => base58.decode(str);

/** @deprecated use toBase58 instead */
export const toB58 = toBase58;

/** @deprecated use fromBase58 instead */
export const fromB58 = fromBase58;
