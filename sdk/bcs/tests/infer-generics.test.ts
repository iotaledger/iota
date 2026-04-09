// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { BcsType } from '../src/bcs-type.js';
import { bcs } from '../src/bcs.js';

import { it, describe, expectTypeOf } from 'vitest';

describe('generic bcs helpers should infer type correctly', () => {
    it('option is typed properly', () => {
        function TestStruct<T extends BcsType<any>>(...typeParameters: [T]) {
            return bcs.struct('TestStruct', {
                value: bcs.option(typeParameters[0]),
            });
        }
        const testStruct = TestStruct(bcs.u8());
        expectTypeOf(testStruct.$inferType).toMatchObjectType<{
            value: number | null;
        }>();
    });

    it('vector is typed properly', () => {
        function TestStruct<T extends BcsType<any>>(...typeParameters: [T]) {
            return bcs.struct('TestStruct', {
                value: bcs.vector(typeParameters[0]),
            });
        }
        const testStruct = TestStruct(bcs.u8());
        expectTypeOf(testStruct.$inferType).toMatchObjectType<{
            value: number[];
        }>();
    });

    it('map is typed properly', () => {
        function TestStruct<K extends BcsType<any>, V extends BcsType<any>>(
            ...typeParameters: [K, V]
        ) {
            return bcs.struct('TestStruct', {
                value: bcs.map(typeParameters[0], typeParameters[1]),
            });
        }
        const testStruct = TestStruct(bcs.u8(), bcs.u64());
        expectTypeOf(testStruct.$inferType).toMatchObjectType<{
            value: Map<number, string>;
        }>();
    });

    it('fixedArray is typed properly', () => {
        function TestStruct<T extends BcsType<any>>(...typeParameters: [T]) {
            return bcs.struct('TestStruct', {
                value: bcs.fixedArray(1, typeParameters[0]),
            });
        }
        const testStruct = TestStruct(bcs.u8());
        expectTypeOf(testStruct.$inferType).toMatchObjectType<{
            value: number[];
        }>();
    });
});
