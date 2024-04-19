// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { fromB64, toB58 } from '@mysten/bcs';
import nacl from 'tweetnacl';
import { describe, expect, it } from 'vitest';

import { decodeSuiPrivateKey } from '../../../src/cryptography/keypair';
import { Ed25519Keypair } from '../../../src/keypairs/ed25519';
import { TransactionBlock } from '../../../src/transactions';
import { verifyPersonalMessage, verifyTransactionBlock } from '../../../src/verify';

const VALID_SECRET_KEY = 'mdqVWeFekT7pqy5T49+tV12jO0m+ESW7ki4zSU9JiCg=';
const PRIVATE_KEY_SIZE = 32;

// Test case generated against rust keytool cli. See https://github.com/MystenLabs/sui/blob/edd2cd31e0b05d336b1b03b6e79a67d8dd00d06b/crates/sui/src/unit_tests/keytool_tests.rs#L165
const TEST_CASES = [
	[
		'film crazy soon outside stand loop subway crumble thrive popular green nuclear struggle pistol arm wife phrase warfare march wheat nephew ask sunny firm',
		'suiprivkey1qrqqxhsu3ndp96644fjk4z5ams5ulgmvprklngt2jhvg2ujn5w4q23vm2y8',
		'0x7ba26cca0b4c313c8d12193263f7df111f32e1835b12204b94e5859187a0aa26',
	],
	[
		'require decline left thought grid priority false tiny gasp angle royal system attack beef setup reward aunt skill wasp tray vital bounce inflict level',
		'suiprivkey1qqcxaf57fnenvflpacacaumf6vl0rt0edddhytanvzhkqhwnjk0zsawjy3x',
		'0x7abd0ad79244dfa245ed4e03b1e5f3295a2332c30f15fb7fa0d3a0f546ca0d74',
	],
	[
		'organ crash swim stick traffic remember army arctic mesh slice swear summer police vast chaos cradle squirrel hood useless evidence pet hub soap lake',
		'suiprivkey1qzq39vxzm0gq7l8dc5dj5allpuww4mavhwhg8mua4cl3lj2c3fvhcsjgphc',
		'0x0f5747833baf8d7794c6a810d74b5bce2cd1286c6634b004bd56a58c92cff7cb',
	],
];

describe('ed25519-keypair', () => {
	it('new keypair', () => {
		const keypair = new Ed25519Keypair();
		expect(keypair.getPublicKey().toRawBytes().length).toBe(32);
		expect(2).toEqual(2);
	});

	it('create keypair from secret key', () => {
		const secretKey = fromB64(VALID_SECRET_KEY);
		const keypair = Ed25519Keypair.fromSecretKey(secretKey);
		expect(keypair.getPublicKey().toBase64()).toEqual(
			'Gy9JCW4+Xb0Pz6nAwM2S2as7IVRLNNXdSmXZi4eLmSI=',
		);
	});

	it('create keypair from secret key and mnemonics matches keytool', () => {
		for (const t of TEST_CASES) {
			// Keypair derived from mnemonic.
			const keypair = Ed25519Keypair.deriveKeypair(t[0]);
			expect(keypair.getPublicKey().toSuiAddress()).toEqual(t[2]);

			// Decode Sui private key from Bech32 string
			const parsed = decodeSuiPrivateKey(t[1]);
			const kp = Ed25519Keypair.fromSecretKey(parsed.secretKey);
			expect(kp.getPublicKey().toSuiAddress()).toEqual(t[2]);

			// Exported keypair matches the Bech32 encoded secret key.
			const exported = kp.export();
			expect(exported.privateKey).toEqual(t[1]);
		}
	});

	it('generate keypair from random seed', () => {
		const keypair = Ed25519Keypair.fromSecretKey(Uint8Array.from(Array(PRIVATE_KEY_SIZE).fill(8)));
		expect(keypair.getPublicKey().toBase64()).toEqual(
			'E5j2LG0aRXxRumpLXz29L2n8qTIWIY3ImX5Ba9F9k8o=',
		);
	});

	it('signature of data is valid', () => {
		const keypair = new Ed25519Keypair();
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

	it('incorrect coin type node for ed25519 derivation path', () => {
		const keypair = Ed25519Keypair.deriveKeypair(TEST_CASES[0][0], `m/44'/4218'/0'/0'/0'`);

		const signData = new TextEncoder().encode('hello world');
		const signature = keypair.signData(signData);
		const isValid = nacl.sign.detached.verify(
			signData,
			signature,
			keypair.getPublicKey().toRawBytes(),
		);
		expect(isValid).toBeTruthy();
	});

	it('incorrect coin type node for ed25519 derivation path', () => {
		expect(() => {
			Ed25519Keypair.deriveKeypair(TEST_CASES[0][0], `m/44'/0'/0'/0'/0'`);
		}).toThrow('Invalid derivation path');
	});

	it('incorrect purpose node for ed25519 derivation path', () => {
		expect(() => {
			Ed25519Keypair.deriveKeypair(TEST_CASES[0][0], `m/54'/4218'/0'/0'/0'`);
		}).toThrow('Invalid derivation path');
	});

	it('invalid mnemonics to derive ed25519 keypair', () => {
		expect(() => {
			Ed25519Keypair.deriveKeypair('aaa');
		}).toThrow('Invalid mnemonic');
	});

	it('signs TransactionBlocks', async () => {
		const keypair = new Ed25519Keypair();
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
		const keypair = new Ed25519Keypair();
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
