// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { Ed25519PublicKey } from '../../../src/keypairs/ed25519';

// Test case generated against CLI:
// cargo build --bin iota
// ../iota/target/debug/iota client new-address --key-scheme ed25519
// ../iota/target/debug/iota keytool list
const TEST_CASES = [
    {
        rawPublicKey: 'UdGRWooy48vGTs0HBokIis5NK+DUjiWc9ENUlcfCCBE=',
        iotaPublicKey: 'AFHRkVqKMuPLxk7NBwaJCIrOTSvg1I4lnPRDVJXHwggR',
        iotaAddress: '0xd857c1a13dcb1eba0efbe12014c44cb319cfb4f19ac4850c82a55b30b430c00b',
    },
    {
        rawPublicKey: '0PTAfQmNiabgbak9U/stWZzKc5nsRqokda2qnV2DTfg=',
        iotaPublicKey: 'AND0wH0JjYmm4G2pPVP7LVmcynOZ7EaqJHWtqp1dg034',
        iotaAddress: '0xa3ebb66759b5aef76a9c7d74e1e32c43b367330c71c8c570185ae6c87bbb9079',
    },
    {
        rawPublicKey: '6L/l0uhGt//9cf6nLQ0+24Uv2qanX/R6tn7lWUJX1Xk=',
        iotaPublicKey: 'AOi/5dLoRrf//XH+py0NPtuFL9qmp1/0erZ+5VlCV9V5',
        iotaAddress: '0x752eea4b000d6e9ac22f60ebb71bfc122713552c1a015ae6fd6ec8acfa7b7ec3',
    },
];

const VALID_KEY_BASE64 = 'Uz39UFseB/B38iBwjesIU1JZxY6y+TRL9P84JFw41W4=';

describe('Ed25519PublicKey', () => {
    it('invalid', () => {
        // public key length 33 is invalid for Ed25519
        expect(() => {
            new Ed25519PublicKey([
                3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                0, 0, 0, 0, 0,
            ]);
        }).toThrow();

        expect(() => {
            new Ed25519PublicKey(
                '0x300000000000000000000000000000000000000000000000000000000000000000000',
            );
        }).toThrow();

        expect(() => {
            new Ed25519PublicKey(
                '0x300000000000000000000000000000000000000000000000000000000000000',
            );
        }).toThrow();

        expect(() => {
            new Ed25519PublicKey(
                '135693854574979916511997248057056142015550763280047535983739356259273198796800000',
            );
        }).toThrow();

        expect(() => {
            new Ed25519PublicKey('12345');
        }).toThrow();
    });

    it('toBase64', () => {
        const key = new Ed25519PublicKey(VALID_KEY_BASE64);
        expect(key.toBase64()).toEqual(VALID_KEY_BASE64);
    });

    it('toBuffer', () => {
        const key = new Ed25519PublicKey(VALID_KEY_BASE64);
        expect(key.toRawBytes().length).toBe(32);
        expect(new Ed25519PublicKey(key.toRawBytes()).equals(key)).toBe(true);
    });

    TEST_CASES.forEach(({ rawPublicKey, iotaPublicKey, iotaAddress }) => {
        it(`toIotaAddress from base64 public key ${iotaAddress}`, () => {
            const key = new Ed25519PublicKey(rawPublicKey);
            expect(key.toIotaAddress()).toEqual(iotaAddress);
        });

        it(`toIotaPublicKey from base64 public key ${iotaAddress}`, () => {
            const key = new Ed25519PublicKey(rawPublicKey);
            expect(key.toIotaPublicKey()).toEqual(iotaPublicKey);
        });
    });
});
