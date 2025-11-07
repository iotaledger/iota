// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { bcs, fromBase64, toBase64 } from './../src/index';

describe('BCS: Primitives', () => {
    it('should support growing size', () => {
        const Coin = bcs.struct('Coin', {
            value: bcs.u64(),
            owner: bcs.string(),
            is_locked: bcs.bool(),
        });

        const rustBcs = 'gNGxBWAAAAAOQmlnIFdhbGxldCBHdXkA';
        const expected = {
            owner: 'Big Wallet Guy',
            value: '412412400000',
            is_locked: false,
        };

        const setBytes = Coin.serialize(expected, { initialSize: 1, maxSize: 1024 });

        expect(Coin.parse(fromBase64(rustBcs))).toEqual(expected);
        expect(setBytes.toBase64()).toEqual(rustBcs);
    });

    it('should error when attempting to grow beyond the allowed size', () => {
        const Coin = bcs.struct('Coin', {
            value: bcs.u64(),
            owner: bcs.string(),
            is_locked: bcs.bool(),
        });

        const expected = {
            owner: 'Big Wallet Guy',
            value: 412412400000n,
            is_locked: false,
        };

        expect(() => Coin.serialize(expected, { initialSize: 1, maxSize: 1 })).toThrowError();
    });

    it('should work when underlying buffer offset is not 0', () => {
        const Coin = bcs.struct('Coin', {
            value: bcs.u64(),
            owner: bcs.string(),
            is_locked: bcs.bool(),
        });

        const rustBcs = 'gNGxBWAAAAAOQmlnIFdhbGxldCBHdXkA';
        const bytes = fromBase64(rustBcs);

        const buffer = new ArrayBuffer(bytes.length + 10);
        const array = new Uint8Array(buffer, 10, bytes.length);
        for (let i = 0; i < bytes.length; i++) {
            array[i] = bytes[i];
        }

        expect(toBase64(array)).toEqual(rustBcs);
        expect(toBase64(new Uint8Array(buffer).slice(10))).toEqual(rustBcs);

        expect(Coin.parse(array)).toEqual({
            owner: 'Big Wallet Guy',
            value: '412412400000',
            is_locked: false,
        });
    });
});
