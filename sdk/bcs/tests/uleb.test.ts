// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { ulebDecode, ulebEncode } from '../src/uleb.js';

describe('ULEB Encoding and Decoding', () => {
    describe('ulebEncode', () => {
        it('should encode zero', () => {
            expect(ulebEncode(0)).toEqual([0]);
        });

        it('should encode small positive numbers', () => {
            expect(ulebEncode(1)).toEqual([1]);
            expect(ulebEncode(127)).toEqual([127]);
        });

        it('should encode multi-byte numbers', () => {
            expect(ulebEncode(128)).toEqual([0x80, 0x01]);
            expect(ulebEncode(129)).toEqual([0x81, 0x01]);
            expect(ulebEncode(255)).toEqual([0xff, 0x01]);
            expect(ulebEncode(300)).toEqual([0xac, 0x02]);
        });

        it('should encode large numbers correctly', () => {
            // 2^14 = 16384
            expect(ulebEncode(16384)).toEqual([0x80, 0x80, 0x01]);
            // 2^21 = 2097152
            expect(ulebEncode(2097152)).toEqual([0x80, 0x80, 0x80, 0x01]);
        });

        it('should encode 2^31', () => {
            // 2^31 = 2147483648
            expect(ulebEncode(2147483648)).toEqual([0x80, 0x80, 0x80, 0x80, 0x08]);
        });

        it('should encode 2^32 - 1', () => {
            // 4294967295
            expect(ulebEncode(4294967295)).toEqual([0xff, 0xff, 0xff, 0xff, 0x0f]);
        });

        it('should encode 2^32', () => {
            // 4294967296
            expect(ulebEncode(4294967296)).toEqual([0x80, 0x80, 0x80, 0x80, 0x10]);
        });

        it('should encode 2^40 - 1', () => {
            // 1099511627775
            expect(ulebEncode(1099511627775)).toEqual([0xff, 0xff, 0xff, 0xff, 0xff, 0x1f]);
        });

        it('should encode 2^53 - 1 (MAX_SAFE_INTEGER)', () => {
            // 9007199254740991
            expect(ulebEncode(Number.MAX_SAFE_INTEGER)).toEqual([
                0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0f,
            ]);
        });
    });

    describe('ulebDecode', () => {
        it('should decode zero', () => {
            const result = ulebDecode([0]);
            expect(result.value).toBe(0);
            expect(result.length).toBe(1);
        });

        it('should decode small positive numbers', () => {
            const result1 = ulebDecode([1]);
            expect(result1.value).toBe(1);
            expect(result1.length).toBe(1);

            const result127 = ulebDecode([127]);
            expect(result127.value).toBe(127);
            expect(result127.length).toBe(1);
        });

        it('should decode multi-byte numbers', () => {
            const result128 = ulebDecode([0x80, 0x01]);
            expect(result128.value).toBe(128);
            expect(result128.length).toBe(2);

            const result129 = ulebDecode([0x81, 0x01]);
            expect(result129.value).toBe(129);
            expect(result129.length).toBe(2);

            const result255 = ulebDecode([0xff, 0x01]);
            expect(result255.value).toBe(255);
            expect(result255.length).toBe(2);

            const result300 = ulebDecode([0xac, 0x02]);
            expect(result300.value).toBe(300);
            expect(result300.length).toBe(2);
        });

        it('should decode large numbers correctly', () => {
            const result16384 = ulebDecode([0x80, 0x80, 0x01]);
            expect(result16384.value).toBe(16384);
            expect(result16384.length).toBe(3);

            const result2097152 = ulebDecode([0x80, 0x80, 0x80, 0x01]);
            expect(result2097152.value).toBe(2097152);
            expect(result2097152.length).toBe(4);
        });

        it('should throw on malformed input (buffer overflow)', () => {
            // [0x80] indicates more bytes follow, but buffer ends
            expect(() => ulebDecode([0x80])).toThrow('ULEB decode error: buffer overflow');

            // [0x81] also indicates more bytes follow
            expect(() => ulebDecode([0x81])).toThrow('ULEB decode error: buffer overflow');

            // [0xFF] indicates more bytes follow
            expect(() => ulebDecode([0xff])).toThrow('ULEB decode error: buffer overflow');

            // Multiple continuation bytes without termination
            expect(() => ulebDecode([0x80, 0x80])).toThrow('ULEB decode error: buffer overflow');
        });

        it('should return correct length for encoded data', () => {
            // The length field represents the number of bytes consumed from the buffer
            const result1 = ulebDecode([1]);
            expect(result1.length).toBe(1);

            const result2 = ulebDecode([0x80, 0x01]);
            expect(result2.length).toBe(2);

            const result3 = ulebDecode([0x80, 0x80, 0x01]);
            expect(result3.length).toBe(3);

            // When there's extra data after the encoded value, length should still be correct
            const resultWithExtra = ulebDecode([0x80, 0x01, 0xff, 0xff]);
            expect(resultWithExtra.value).toBe(128);
            expect(resultWithExtra.length).toBe(2); // Only consumed 2 bytes
        });

        it('should handle Uint8Array input', () => {
            const result = ulebDecode(new Uint8Array([0x80, 0x01]));
            expect(result.value).toBe(128);
            expect(result.length).toBe(2);
        });

        it('should decode 2^31', () => {
            // 2^31 = 2147483648
            const result = ulebDecode([0x80, 0x80, 0x80, 0x80, 0x08]);
            expect(result.value).toBe(2147483648);
            expect(result.length).toBe(5);
        });

        it('should decode 2^32 - 1', () => {
            // 4294967295
            const result = ulebDecode([0xff, 0xff, 0xff, 0xff, 0x0f]);
            expect(result.value).toBe(4294967295);
            expect(result.length).toBe(5);
        });

        it('should decode 2^32', () => {
            // 4294967296
            const result = ulebDecode([0x80, 0x80, 0x80, 0x80, 0x10]);
            expect(result.value).toBe(4294967296);
            expect(result.length).toBe(5);
        });

        it('should decode 2^40 - 1', () => {
            // 1099511627775
            const result = ulebDecode([0xff, 0xff, 0xff, 0xff, 0xff, 0x1f]);
            expect(result.value).toBe(1099511627775);
            expect(result.length).toBe(6);
        });

        it('should decode 2^53 - 1 (MAX_SAFE_INTEGER)', () => {
            // 9007199254740991
            const result = ulebDecode([0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0xff, 0x0f]);
            expect(result.value).toBe(Number.MAX_SAFE_INTEGER);
            expect(result.length).toBe(8);
        });

        it('should decode valid multi-byte sequences from issue reproduction', () => {
            const result1 = ulebDecode([0x80, 0x00]);
            expect(result1.value).toBe(0);
            expect(result1.length).toBe(2);

            const result2 = ulebDecode([0xff, 0xff, 0xff, 0xff, 0x07]);
            expect(result2.value).toBe(2147483647);
            expect(result2.length).toBe(5);

            const result3 = ulebDecode([0xff, 0xff, 0xff, 0xff, 0x0f]);
            expect(result3.value).toBe(4294967295);
            expect(result3.length).toBe(5);

            const result4 = ulebDecode([0xff, 0xff, 0xff, 0xff, 0x1f]);
            expect(result4.value).toBe(8589934591);
            expect(result4.length).toBe(5);
        });
    });

    describe('round-trip encoding/decoding', () => {
        it('should round-trip encode and decode various values', () => {
            const testValues = [
                0,
                1,
                127,
                128,
                129,
                255,
                256,
                300,
                1000,
                16384,
                65535,
                1000000,
                2097152,
                2147483648, // 2^31
                4294967295, // 2^32 - 1
                4294967296, // 2^32
                1099511627775, // 2^40 - 1
                Number.MAX_SAFE_INTEGER, // 2^53 - 1
            ];

            for (const value of testValues) {
                const encoded = ulebEncode(value);
                const decoded = ulebDecode(encoded);
                expect(decoded.value).toBe(value);
                expect(decoded.length).toBe(encoded.length);
            }
        });

        it('should correctly report consumed bytes when buffer has extra data', () => {
            const encoded = ulebEncode(300);
            const withExtra = [...encoded, 0xaa, 0xbb, 0xcc];

            const result = ulebDecode(withExtra);
            expect(result.value).toBe(300);
            expect(result.length).toBe(encoded.length);
        });
    });

    describe('malformed input handling', () => {
        it('should throw on empty buffer', () => {
            expect(() => ulebDecode([])).toThrow('ULEB decode error: buffer overflow');
        });

        it('should throw on continuation byte without termination', () => {
            expect(() => ulebDecode([0x80])).toThrow('ULEB decode error: buffer overflow');
            expect(() => ulebDecode([0x81])).toThrow('ULEB decode error: buffer overflow');
            expect(() => ulebDecode([0xff])).toThrow('ULEB decode error: buffer overflow');
            expect(() => ulebDecode([0x80, 0x80])).toThrow('ULEB decode error: buffer overflow');
        });
    });
});
