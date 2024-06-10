// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { BCS, getIotaMoveConfig } from '../src/index';
import { serde } from './utils';

describe('BCS: Inline struct definitions', () => {
    it('should de/serialize inline definition', () => {
        const bcs = new BCS(getIotaMoveConfig());

        // reported by kklas: vector<T> returns [undefined]
        bcs.registerStructType(['FooType', 'T'], {
            generic_vec: ['vector', 'T'],
        });

        const value = { generic_vec: ['1', '2', '3'] };
        expect(serde(bcs, ['FooType', 'u64'], value)).toEqual(value);
    });
});
