// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, test } from 'vitest';

import { isValidIotaNSName, normalizeIotaNSName } from '../../../src/utils';

describe('isValidIotaNSName', () => {
    test('valid IotaNS names', () => {
        expect(isValidIotaNSName('example.iota')).toBe(true);
        expect(isValidIotaNSName('EXAMPLE.iota')).toBe(true);
        expect(isValidIotaNSName('@example')).toBe(true);
        expect(isValidIotaNSName('1.example.iota')).toBe(true);
        expect(isValidIotaNSName('1@example')).toBe(true);
        expect(isValidIotaNSName('a.b.c.example.iota')).toBe(true);
        expect(isValidIotaNSName('A.B.c.123@Example')).toBe(true);
        expect(isValidIotaNSName('1-a@1-b')).toBe(true);
        expect(isValidIotaNSName('1-a.1-b.iota')).toBe(true);
        expect(isValidIotaNSName('-@test')).toBe(false);
        expect(isValidIotaNSName('-1@test')).toBe(false);
        expect(isValidIotaNSName('test@-')).toBe(false);
        expect(isValidIotaNSName('test@-1')).toBe(false);
        expect(isValidIotaNSName('test@-a')).toBe(false);
        expect(isValidIotaNSName('test.iota2')).toBe(false);
        expect(isValidIotaNSName('.iota2')).toBe(false);
        expect(isValidIotaNSName('test@')).toBe(false);
        expect(isValidIotaNSName('@@')).toBe(false);
        expect(isValidIotaNSName('@@test')).toBe(false);
        expect(isValidIotaNSName('test@test.test')).toBe(false);
        expect(isValidIotaNSName('@test.test')).toBe(false);
        expect(isValidIotaNSName('#@test')).toBe(false);
        expect(isValidIotaNSName('test@#')).toBe(false);
        expect(isValidIotaNSName('test.#.iota')).toBe(false);
        expect(isValidIotaNSName('#.iota')).toBe(false);
        expect(isValidIotaNSName('@.test.sue')).toBe(false);

        expect(isValidIotaNSName('hello-.iota')).toBe(false);
        expect(isValidIotaNSName('hello--.iota')).toBe(false);
        expect(isValidIotaNSName('hello.-iota')).toBe(false);
        expect(isValidIotaNSName('hello.--iota')).toBe(false);
        expect(isValidIotaNSName('hello.iota-')).toBe(false);
        expect(isValidIotaNSName('hello.iota--')).toBe(false);
        expect(isValidIotaNSName('hello-@iota')).toBe(false);
        expect(isValidIotaNSName('hello--@iota')).toBe(false);
        expect(isValidIotaNSName('hello@-iota')).toBe(false);
        expect(isValidIotaNSName('hello@--iota')).toBe(false);
        expect(isValidIotaNSName('hello@iota-')).toBe(false);
        expect(isValidIotaNSName('hello@iota--')).toBe(false);
        expect(isValidIotaNSName('hello--world@iota')).toBe(false);
    });
});

describe('normalizeIotaNSName', () => {
    test('normalize IotaNS names', () => {
        expect(normalizeIotaNSName('example.iota')).toMatch('@example');
        expect(normalizeIotaNSName('EXAMPLE.iota')).toMatch('@example');
        expect(normalizeIotaNSName('@example')).toMatch('@example');
        expect(normalizeIotaNSName('1.example.iota')).toMatch('1@example');
        expect(normalizeIotaNSName('1@example')).toMatch('1@example');
        expect(normalizeIotaNSName('a.b.c.example.iota')).toMatch('a.b.c@example');
        expect(normalizeIotaNSName('A.B.c.123@Example')).toMatch('a.b.c.123@example');
        expect(normalizeIotaNSName('1-a@1-b')).toMatch('1-a@1-b');
        expect(normalizeIotaNSName('1-a.1-b.iota')).toMatch('1-a@1-b');

        expect(normalizeIotaNSName('example.iota', 'dot')).toMatch('example.iota');
        expect(normalizeIotaNSName('EXAMPLE.iota', 'dot')).toMatch('example.iota');
        expect(normalizeIotaNSName('@example', 'dot')).toMatch('example.iota');
        expect(normalizeIotaNSName('1.example.iota', 'dot')).toMatch('1.example.iota');
        expect(normalizeIotaNSName('1@example', 'dot')).toMatch('1.example.iota');
        expect(normalizeIotaNSName('a.b.c.example.iota', 'dot')).toMatch('a.b.c.example.iota');
        expect(normalizeIotaNSName('A.B.c.123@Example', 'dot')).toMatch('a.b.c.123.example.iota');
        expect(normalizeIotaNSName('1-a@1-b', 'dot')).toMatch('1-a.1-b.iota');
        expect(normalizeIotaNSName('1-a.1-b.iota', 'dot')).toMatch('1-a.1-b.iota');

        expect(() => normalizeIotaNSName('-@test')).toThrowError('Invalid IotaNS name -@test');
        expect(normalizeIotaNSName('1-a@1-b')).toMatchInlineSnapshot('"1-a@1-b"');
        expect(normalizeIotaNSName('1-a.1-b.iota')).toMatchInlineSnapshot('"1-a@1-b"');
        expect(() => normalizeIotaNSName('-@test')).toThrowError('Invalid IotaNS name -@test');
        expect(() => normalizeIotaNSName('-1@test')).toThrowError('Invalid IotaNS name -1@test');
        expect(() => normalizeIotaNSName('test@-')).toThrowError('Invalid IotaNS name test@-');
        expect(() => normalizeIotaNSName('test@-1')).toThrowError('Invalid IotaNS name test@-1');
        expect(() => normalizeIotaNSName('test@-a')).toThrowError('Invalid IotaNS name test@-a');
        expect(() => normalizeIotaNSName('test.iota2')).toThrowError(
            'Invalid IotaNS name test.iota2',
        );
        expect(() => normalizeIotaNSName('.iota2')).toThrowError('Invalid IotaNS name .iota2');
        expect(() => normalizeIotaNSName('test@')).toThrowError('Invalid IotaNS name test@');
        expect(() => normalizeIotaNSName('@@')).toThrowError('Invalid IotaNS name @@');
        expect(() => normalizeIotaNSName('@@test')).toThrowError('Invalid IotaNS name @@test');
        expect(() => normalizeIotaNSName('test@test.test')).toThrowError(
            'Invalid IotaNS name test@test.test',
        );
        expect(() => normalizeIotaNSName('@test.test')).toThrowError(
            'Invalid IotaNS name @test.test',
        );
        expect(() => normalizeIotaNSName('#@test')).toThrowError('Invalid IotaNS name #@test');
        expect(() => normalizeIotaNSName('test@#')).toThrowError('Invalid IotaNS name test@#');
        expect(() => normalizeIotaNSName('test.#.iota')).toThrowError(
            'Invalid IotaNS name test.#.iota',
        );
        expect(() => normalizeIotaNSName('#.iota')).toThrowError('Invalid IotaNS name #.iota');
        expect(() => normalizeIotaNSName('@.test.sue')).toThrowError(
            'Invalid IotaNS name @.test.sue',
        );
    });
});
