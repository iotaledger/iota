// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toB64, toHEX } from '@iota/bcs';
import { describe, expect, it } from 'vitest';

import { Secp256r1PublicKey } from '../../../src/keypairs/secp256r1/publickey';
import { INVALID_SECP256R1_PUBLIC_KEY, VALID_SECP256R1_PUBLIC_KEY } from './secp256r1-keypair.test';

// Test case generated against CLI:
// cargo build --bin iota
// ../iota/target/debug/iota client new-address --key-scheme secp256r1
// ../iota/target/debug/iota keytool list
const TEST_CASES = [
    {
        rawPublicKey: 'A8Ju2r5X3EZ3aYuZzH+Ofs6cd1j2WOwY7lhoJQenulBl',
        iotaPublicKey: 'AgPCbtq+V9xGd2mLmcx/jn7OnHdY9ljsGO5YaCUHp7pQZQ==',
        iotaAddress: '0xafd0f5a4f41c5770c201879518740b83743164ed2445016fbba9ae98e04af8a5',
    },
];

describe('Secp256r1PublicKey', () => {
    it('invalid', () => {
        expect(() => {
            new Secp256r1PublicKey(INVALID_SECP256R1_PUBLIC_KEY);
        }).toThrow();

        expect(() => {
            const invalid_pubkey_buffer = new Uint8Array(INVALID_SECP256R1_PUBLIC_KEY);
            const invalid_pubkey_base64 = toB64(invalid_pubkey_buffer);
            new Secp256r1PublicKey(invalid_pubkey_base64);
        }).toThrow();

        expect(() => {
            const pubkey_buffer = new Uint8Array(VALID_SECP256R1_PUBLIC_KEY);
            const wrong_encode = toHEX(pubkey_buffer);
            new Secp256r1PublicKey(wrong_encode);
        }).toThrow();

        expect(() => {
            new Secp256r1PublicKey('12345');
        }).toThrow();
    });

    it('toBase64', () => {
        const pub_key = new Uint8Array(VALID_SECP256R1_PUBLIC_KEY);
        const pub_key_base64 = toB64(pub_key);
        const key = new Secp256r1PublicKey(pub_key_base64);
        expect(key.toBase64()).toEqual(pub_key_base64);
    });

    it('toBuffer', () => {
        const pub_key = new Uint8Array(VALID_SECP256R1_PUBLIC_KEY);
        const pub_key_base64 = toB64(pub_key);
        const key = new Secp256r1PublicKey(pub_key_base64);
        expect(key.toRawBytes().length).toBe(33);
        expect(new Secp256r1PublicKey(key.toRawBytes()).equals(key)).toBe(true);
    });

    TEST_CASES.forEach(({ rawPublicKey, iotaPublicKey, iotaAddress }) => {
        it(`toIotaAddress from base64 public key ${iotaAddress}`, () => {
            const key = new Secp256r1PublicKey(rawPublicKey);
            expect(key.toIotaAddress()).toEqual(iotaAddress);
        });

        it(`toIotaPublicKey from base64 public key ${iotaAddress}`, () => {
            const key = new Secp256r1PublicKey(rawPublicKey);
            expect(key.toIotaPublicKey()).toEqual(iotaPublicKey);
        });
    });
});
