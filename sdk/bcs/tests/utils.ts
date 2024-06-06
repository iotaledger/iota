// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BCS } from '../src/index.js';

/** Serialize and deserialize the result. */
export function serde(bcs: BCS, type: any, data: any): any {
    const ser = bcs.ser(type, data).toString('hex');
    const de = bcs.de(type, ser, 'hex');
    return de;
}
