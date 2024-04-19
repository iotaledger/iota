// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { fromB64, toB58 } from '@mysten/bcs';
import nacl from 'tweetnacl';
import { describe, expect, it } from 'vitest';

import { decodeSuiPrivateKey } from '../../../src/cryptography/keypair';
import { Ed25519LegacyKeypair } from '../../../src/keypairs/ed25519legacy';
import { TransactionBlock } from '../../../src/transactions';
import { verifyPersonalMessage, verifyTransactionBlock } from '../../../src/verify';

const VALID_SECRET_KEY = 'mdqVWeFekT7pqy5T49+tV12jO0m+ESW7ki4zSU9JiCg=';
const PRIVATE_KEY_SIZE = 32;

// Test case generated against rust keytool cli. See https://github.com/MystenLabs/sui/blob/edd2cd31e0b05d336b1b03b6e79a67d8dd00d06b/crates/sui/src/unit_tests/keytool_tests.rs#L165
const TEST_CASES = [
	[
		'film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm',
		'suiprivkey1llqqxhsu3ndp96644fjk4z5ams5ulgmvprklngt2jhvg2ujn5w4q2c9ha8m',
		'0x77a58912dac82dcae215d6910ee8ae5e79d4b76555ac0f505c188e91b49cddb3',
	],
	[
		'require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level',
		'suiprivkey1lucxaf57fnenvflpacacaumf6vl0rt0edddhytanvzhkqhwnjk0zs587nj6',
		'0x58357bc75b7f88132b50040cf5f04cd53710f349fe0095c3027bc2657c589962',
	],
	[
		'organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake',
		'suiprivkey1l7q39vxzm0gq7l8dc5dj5allpuww4mavhwhg8mua4cl3lj2c3fvhcemyk5y',
		'0x8c1eea724b985d649790301a08481b581b42c20c5c044b7c4a44c611b8bbcd13',
	],
];

describe('ed25519legacy-keypair', () => {
	it('new keypair', () => {
		const keypair = new Ed25519LegacyKeypair();
		expect(keypair.getPublicKey().toRawBytes().length).toBe(32);
		expect(2).toEqual(2);
	});

	it('create keypair from secret key', () => {
		const secretKey = fromB64(VALID_SECRET_KEY);
		const keypair = Ed25519LegacyKeypair.fromSecretKey(secretKey);
		expect(keypair.getPublicKey().toBase64()).toEqual(
			'Gy9JCW4+Xb0Pz6nAwM2S2as7IVRLNNXdSmXZi4eLmSI=',
		);
	});

	it('create keypair from secret key and mnemonics matches keytool', () => {
		for (const t of TEST_CASES) {
			// Keypair derived from mnemonic.
			const keypair = Ed25519LegacyKeypair.deriveKeypair(t[0]);
			expect(keypair.getPublicKey().toSuiAddress()).toEqual(t[2]);

			// Decode Sui private key from Bech32 string
			const parsed = decodeSuiPrivateKey(t[1]);
			const kp = Ed25519LegacyKeypair.fromSecretKey(parsed.secretKey);
			expect(kp.getPublicKey().toSuiAddress()).toEqual(t[2]);

			// Exported keypair matches the Bech32 encoded secret key.
			const exported = kp.export();
			expect(exported.privateKey).toEqual(t[1]);
		}
	});

	it('generate keypair from random seed', () => {
		const keypair = Ed25519LegacyKeypair.fromSecretKey(
			Uint8Array.from(Array(PRIVATE_KEY_SIZE).fill(8)),
		);
		expect(keypair.getPublicKey().toBase64()).toEqual(
			'E5j2LG0aRXxRumpLXz29L2n8qTIWIY3ImX5Ba9F9k8o=',
		);
	});

	it('signature of data is valid', () => {
		const keypair = new Ed25519LegacyKeypair();
		const signData = new TextEncoder().encode('hello world');
		const signature = keypair.signData(signData);
		const isValid = nacl.sign.detached.verify(
			signData,
			signature,
			keypair.getPublicKey().toRawBytes(),
		);
		expect(isValid).toBeTruthy();
		expect(keypair.getPublicKey().verify(signData, signature));
	});

	it('incorrect coin type node for ed25519Legacy derivation path', () => {
		const keypair = Ed25519LegacyKeypair.deriveKeypair(TEST_CASES[0][0], `m/44'/4218'/0'/0'/0'`);

		const signData = new TextEncoder().encode('hello world');
		const signature = keypair.signData(signData);
		const isValid = nacl.sign.detached.verify(
			signData,
			signature,
			keypair.getPublicKey().toRawBytes(),
		);
		expect(isValid).toBeTruthy();
	});

	it('incorrect coin type node for ed25519Legacy derivation path', () => {
		expect(() => {
			Ed25519LegacyKeypair.deriveKeypair(TEST_CASES[0][0], `m/44'/0'/0'/0'/0'`);
		}).toThrow('Invalid derivation path');
	});

	it('incorrect purpose node for ed25519Legacy derivation path', () => {
		expect(() => {
			Ed25519LegacyKeypair.deriveKeypair(TEST_CASES[0][0], `m/54'/4218'/0'/0'/0'`);
		}).toThrow('Invalid derivation path');
	});

	it('invalid mnemonics to derive ed25519Legacy keypair', () => {
		expect(() => {
			Ed25519LegacyKeypair.deriveKeypair('aaa');
		}).toThrow('Invalid mnemonic');
	});

	it('signs TransactionBlocks', async () => {
		const keypair = new Ed25519LegacyKeypair();
		const txb = new TransactionBlock();
		txb.setSender(keypair.getPublicKey().toSuiAddress());
		txb.setGasPrice(5);
		txb.setGasBudget(100);
		txb.setGasPayment([
			{
				objectId: (Math.random() * 100000).toFixed(0).padEnd(64, '0'),
				version: String((Math.random() * 10000).toFixed(0)),
				digest: toB58(new Uint8Array([0, 1, 2, 3, 4, 5, 6, 7, 8, 9, 0, 1, 2, 3, 4, 5, 6, 7, 8, 9])),
			},
		]);

		const bytes = await txb.build();

		const serializedSignature = (await keypair.signTransactionBlock(bytes)).signature;

		expect(await keypair.getPublicKey().verifyTransactionBlock(bytes, serializedSignature)).toEqual(
			true,
		);
		expect(await keypair.getPublicKey().verifyTransactionBlock(bytes, serializedSignature)).toEqual(
			true,
		);
		expect(!!(await verifyTransactionBlock(bytes, serializedSignature))).toEqual(true);
	});

	it('signs PersonalMessages', async () => {
		const keypair = new Ed25519LegacyKeypair();
		const message = new TextEncoder().encode('hello world');

		const serializedSignature = (await keypair.signPersonalMessage(message)).signature;

		expect(
			await keypair.getPublicKey().verifyPersonalMessage(message, serializedSignature),
		).toEqual(true);
		expect(
			await keypair.getPublicKey().verifyPersonalMessage(message, serializedSignature),
		).toEqual(true);
		expect(!!(await verifyPersonalMessage(message, serializedSignature))).toEqual(true);
	});
});
