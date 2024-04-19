// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import type { SignatureScheme } from '../../cryptography/signature-scheme.js';
import { Ed25519Keypair } from '../ed25519/keypair.js';
import { Ed25519LegacyPublicKey } from './publickey.js';

/**
 * An Ed25519Legacy Keypair used for signing transactions.
 */
export class Ed25519LegacyKeypair extends Ed25519Keypair {
	/**
	 * Get the key scheme of the keypair ED25519Legacy
	 */
	getKeyScheme(): SignatureScheme {
		return 'ED25519Legacy';
	}

	/**
	 * Create a new Ed25519 Legacy keypair instance
	 * from a provided {@link Ed25519Keypair}.
	 *
	 * @param ed25519keypair Ed25519 keypair
	 */
	static fromEd25519Keypair(ed25519keypair: Ed25519Keypair): Ed25519LegacyKeypair {
		const secretKey = ed25519keypair.getRawSecretKey();
		const publicKey = ed25519keypair.getPublicKey().toRawBytes();
		const keypair = {
			publicKey,
			secretKey,
		};
		return new Ed25519LegacyKeypair(keypair);
	}

	/**
	 * Generate a new random Ed25519 keypair
	 */
	static generate(): Ed25519LegacyKeypair {
		return Ed25519LegacyKeypair.fromEd25519Keypair(super.generate());
	}

	/**
	 * Create a Ed25519 Legacy keypair from a raw secret key byte array, also known as seed.
	 * This is NOT the private scalar which is result of hashing and bit clamping of
	 * the raw secret key.
	 *
	 * @throws error if the provided secret key is invalid and validation is not skipped.
	 *
	 * @param secretKey secret key byte array
	 * @param options: skip secret key validation
	 */
	static fromSecretKey(
		secretKey: Uint8Array,
		options?: { skipValidation?: boolean },
	): Ed25519LegacyKeypair {
		return Ed25519LegacyKeypair.fromEd25519Keypair(super.fromSecretKey(secretKey, options));
	}

	/**
	 * The public key for this Ed25519 Legacy keypair
	 */
	getPublicKey(): Ed25519LegacyPublicKey {
		return Ed25519LegacyPublicKey.fromEd25519PublicKey(super.getPublicKey());
	}

	/**
	 * Derive Ed25519 Legacy keypair from mnemonics and path. The mnemonics must be normalized
	 * and validated against the english wordlist.
	 *
	 * If path is none, it will default to m/44'/4218'/0'/0'/0', otherwise the path must
	 * be compliant to SLIP-0010 in form m/44'/4218'/{account_index}'/{change_index}'/{address_index}'.
	 */
	static deriveKeypair(mnemonics: string, path?: string): Ed25519LegacyKeypair {
		return Ed25519LegacyKeypair.fromEd25519Keypair(super.deriveKeypair(mnemonics, path));
	}

	/**
	 * Derive Ed25519 Legacy keypair from mnemonicSeed and path.
	 *
	 * If path is none, it will default to m/44'/4218'/0'/0'/0', otherwise the path must
	 * be compliant to SLIP-0010 in form m/44'/4218'/{account_index}'/{change_index}'/{address_index}'.
	 */
	static deriveKeypairFromSeed(seedHex: string, path?: string): Ed25519LegacyKeypair {
		return Ed25519LegacyKeypair.fromEd25519Keypair(super.deriveKeypairFromSeed(seedHex, path));
	}
}
