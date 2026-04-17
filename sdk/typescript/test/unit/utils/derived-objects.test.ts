// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, test } from 'vitest';

import { bcs, TypeTagSerializer } from '../../../src/bcs';
import { deriveObjectID } from '../../../src/utils/derived-objects';
import { normalizeIotaAddress } from '../../../src/utils';

// Snapshots are recreated from `derived_object_tests.move` file,
// as well as `crates/iota-types/src/derived_object.rs` file.
describe('derived object test utils', () => {
    test('deriveObjectID with primitive type', () => {
        const key = bcs.byteVector().serialize(new TextEncoder().encode('foo')).toBytes();

        expect(deriveObjectID(normalizeIotaAddress('0x2'), 'vector<u8>', key)).toBe(
            '0xa2b411aa9588c398d8e3bc97dddbdd430b5ded7f81545d05e33916c3ca0f30c3',
        );
        expect(
            deriveObjectID(
                normalizeIotaAddress('0x2'),
                TypeTagSerializer.parseFromStr('vector<u8>'),
                key,
            ),
        ).toBe('0xa2b411aa9588c398d8e3bc97dddbdd430b5ded7f81545d05e33916c3ca0f30c3');
    });

    test('deriveObjectID with struct type', () => {
        const structType = bcs.struct('DemoStruct', {
            value: bcs.u64(),
        });
        const key = structType.serialize({ value: 1 }).toBytes();

        expect(deriveObjectID('0x2', `0x2::derived_object_tests::DemoStruct`, key)).toBe(
            '0x20c58d8790a5d2214c159c23f18a5fdc347211e511186353e785ad543abcea6b',
        );
        expect(
            deriveObjectID(
                '0x2',
                TypeTagSerializer.parseFromStr('0x2::derived_object_tests::DemoStruct'),
                key,
            ),
        ).toBe('0x20c58d8790a5d2214c159c23f18a5fdc347211e511186353e785ad543abcea6b');
    });

    test('deriveObjectID with nested struct type', () => {
        const structType = bcs.struct('GenericStruct<T>', {
            value: bcs.u64(),
        });
        const key = structType.serialize({ value: 1 }).toBytes();

        expect(deriveObjectID('0x2', `0x2::derived_object_tests::GenericStruct<u64>`, key)).toBe(
            '0xb497b8dcf1e297ae5fa69c040e4a08ef8240d5373bbc9d6b686ffbd7dfe04cbe',
        );
        expect(
            deriveObjectID(
                '0x2',
                TypeTagSerializer.parseFromStr('0x2::derived_object_tests::GenericStruct<u64>'),
                key,
            ),
        ).toBe('0xb497b8dcf1e297ae5fa69c040e4a08ef8240d5373bbc9d6b686ffbd7dfe04cbe');
    });

    test('deriveObjectID with non-generic primitive type', () => {
        const key = bcs.U8.serialize(1).toBytes();

        expect(deriveObjectID('0x2', 'u8', key)).toBe(
            '0x4de7696edddfb592a8dc7c8b66053b1557eb4fa1a9194322562aabf3da9e9239',
        );
        expect(deriveObjectID('0x2', TypeTagSerializer.parseFromStr('u8'), key)).toBe(
            '0x4de7696edddfb592a8dc7c8b66053b1557eb4fa1a9194322562aabf3da9e9239',
        );
    });
});
