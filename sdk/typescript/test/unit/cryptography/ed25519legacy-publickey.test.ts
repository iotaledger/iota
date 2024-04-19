// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { describe, expect, it } from 'vitest';

import { Ed25519LegacyPublicKey } from '../../../src/keypairs/ed25519legacy';

// Test case generated against CLI:
// cargo build --bin sui
// ../sui/target/debug/sui client new-address ed25519
// ../sui/target/debug/sui keytool list
const TEST_CASES = [
	{
		rawPublicKey: 'UdGRWooy48vGTs0HBokIis5NK+DUjiWc9ENUlcfCCBE=',
		suiPublicKey: '/1HRkVqKMuPLxk7NBwaJCIrOTSvg1I4lnPRDVJXHwggR',
		suiAddress: '0xdd18040267df3999641fc476fe280ec6d2d9b3529ea1de3c8ca2aba2af4d7ae3',
	},
	{
		rawPublicKey: '0PTAfQmNiabgbak9U/stWZzKc5nsRqokda2qnV2DTfg=',
		suiPublicKey: '/9D0wH0JjYmm4G2pPVP7LVmcynOZ7EaqJHWtqp1dg034',
		suiAddress: '0x4780eadec9609b08c30d8dc2d272fddfa5fcf20f03fb8b879affae3df2b1019d',
	},
	{
		rawPublicKey: '6L/l0uhGt//9cf6nLQ0+24Uv2qanX/R6tn7lWUJX1Xk=',
		suiPublicKey: '/+i/5dLoRrf//XH+py0NPtuFL9qmp1/0erZ+5VlCV9V5',
		suiAddress: '0xaa5b145883e842c86a5f26b2abffeea2aa2e69e9b606296e7c40b68199b90f8d',
	},
];

const VALID_KEY_BASE64 = 'Uz39UFseB/B38iBwjesIU1JZxY6y+TRL9P84JFw41W4=';

describe('Ed25519LegacyPublicKey', () => {
	it('invalid', () => {
		// public key length 33 is invalid for Ed25519
		expect(() => {
			new Ed25519LegacyPublicKey([
				3, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
				0, 0,
			]);
		}).toThrow();

		expect(() => {
			new Ed25519LegacyPublicKey(
				'0x300000000000000000000000000000000000000000000000000000000000000000000',
			);
		}).toThrow();

		expect(() => {
			new Ed25519LegacyPublicKey(
				'0x300000000000000000000000000000000000000000000000000000000000000',
			);
		}).toThrow();

		expect(() => {
			new Ed25519LegacyPublicKey(
				'135693854574979916511997248057056142015550763280047535983739356259273198796800000',
			);
		}).toThrow();

		expect(() => {
			new Ed25519LegacyPublicKey('12345');
		}).toThrow();
	});

	it('toBase64', () => {
		const key = new Ed25519LegacyPublicKey(VALID_KEY_BASE64);
		expect(key.toBase64()).toEqual(VALID_KEY_BASE64);
	});

	it('toBuffer', () => {
		const key = new Ed25519LegacyPublicKey(VALID_KEY_BASE64);
		expect(key.toRawBytes().length).toBe(32);
		expect(new Ed25519LegacyPublicKey(key.toRawBytes()).equals(key)).toBe(true);
	});

	TEST_CASES.forEach(({ rawPublicKey, suiPublicKey, suiAddress }) => {
		it(`toSuiAddress from base64 public key ${suiAddress}`, () => {
			const key = new Ed25519LegacyPublicKey(rawPublicKey);
			expect(key.toSuiAddress()).toEqual(suiAddress);
		});

		it(`toSuiPublicKey from base64 public key ${suiAddress}`, () => {
			const key = new Ed25519LegacyPublicKey(rawPublicKey);
			expect(key.toSuiPublicKey()).toEqual(suiPublicKey);
		});
	});
});
