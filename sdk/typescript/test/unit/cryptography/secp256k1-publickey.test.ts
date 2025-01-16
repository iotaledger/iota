// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toB64, toHEX } from '@iota/bcs';
import { describe, expect, it } from 'vitest';

import { Secp256k1PublicKey } from '../../../src/keypairs/secp256k1/publickey';
import { INVALID_SECP256K1_PUBLIC_KEY, VALID_SECP256K1_PUBLIC_KEY } from './secp256k1-keypair.test';

// Test case generated against CLI:
// cargo build --bin iota
// ../iota/target/debug/iota client new-address --key-scheme secp256k1
// ../iota/target/debug/iota keytool list
const TEST_CASES = [
    {
        rawPublicKey: 'AwTC3jVFRxXc3RJIFgoQcv486QdqwYa8vBp4bgSq0gsI',
        iotaPublicKey: 'AQMEwt41RUcV3N0SSBYKEHL+POkHasGGvLwaeG4EqtILCA==',
        iotaAddress: '0xcdce00b4326fb908fdac83c35bcfbda323bfcc0618b47c66ccafbdced850efaa',
    },
    {
        rawPublicKey: 'A1F2CtldIGolO92Pm9yuxWXs5E07aX+6ZEHAnSuKOhii',
        iotaPublicKey: 'AQNRdgrZXSBqJTvdj5vcrsVl7ORNO2l/umRBwJ0rijoYog==',
        iotaAddress: '0xb588e58ed8967b6a6f9dbce76386283d374cf7389fb164189551257e32b023b2',
    },
    {
        rawPublicKey: 'Ak5rsa5Od4T6YFN/V3VIhZ/azMMYPkUilKQwc+RiaId+',
        iotaPublicKey: 'AQJOa7GuTneE+mBTf1d1SIWf2szDGD5FIpSkMHPkYmiHfg==',
        iotaAddress: '0x694dd74af1e82b968822a82fb5e315f6d20e8697d5d03c0b15e0178c1a1fcfa0',
    },
    {
        rawPublicKey: 'A4XbJ3fLvV/8ONsnLHAW1nORKsoCYsHaXv9FK1beMtvY',
        iotaPublicKey: 'AQOF2yd3y71f/DjbJyxwFtZzkSrKAmLB2l7/RStW3jLb2A==',
        iotaAddress: '0x78acc6ca0003457737d755ade25a6f3a144e5e44ed6f8e6af4982c5cc75e55e7',
    },
];

describe('Secp256k1PublicKey', () => {
    it('invalid', () => {
        expect(() => {
            new Secp256k1PublicKey(INVALID_SECP256K1_PUBLIC_KEY);
        }).toThrow();

        expect(() => {
            const invalid_pubkey_buffer = new Uint8Array(INVALID_SECP256K1_PUBLIC_KEY);
            const invalid_pubkey_base64 = toB64(invalid_pubkey_buffer);
            new Secp256k1PublicKey(invalid_pubkey_base64);
        }).toThrow();

        expect(() => {
            const pubkey_buffer = new Uint8Array(VALID_SECP256K1_PUBLIC_KEY);
            const wrong_encode = toHEX(pubkey_buffer);
            new Secp256k1PublicKey(wrong_encode);
        }).toThrow();

        expect(() => {
            new Secp256k1PublicKey('12345');
        }).toThrow();
    });

    it('toBase64', () => {
        const pub_key = new Uint8Array(VALID_SECP256K1_PUBLIC_KEY);
        const pub_key_base64 = toB64(pub_key);
        const key = new Secp256k1PublicKey(pub_key_base64);
        expect(key.toBase64()).toEqual(pub_key_base64);
    });

    it('toBuffer', () => {
        const pub_key = new Uint8Array(VALID_SECP256K1_PUBLIC_KEY);
        const pub_key_base64 = toB64(pub_key);
        const key = new Secp256k1PublicKey(pub_key_base64);
        expect(key.toRawBytes().length).toBe(33);
        expect(new Secp256k1PublicKey(key.toRawBytes()).equals(key)).toBe(true);
    });

    TEST_CASES.forEach(({ rawPublicKey, iotaPublicKey, iotaAddress }) => {
        it(`toIotaAddress from base64 public key ${iotaAddress}`, () => {
            const key = new Secp256k1PublicKey(rawPublicKey);
            expect(key.toIotaAddress()).toEqual(iotaAddress);
        });

        it(`toIotaPublicKey from base64 public key ${iotaAddress}`, () => {
            const key = new Secp256k1PublicKey(rawPublicKey);
            expect(key.toIotaPublicKey()).toEqual(iotaPublicKey);
        });
    });
});
