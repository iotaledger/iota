// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { SIGNATURE_SCHEME_TO_FLAG } from '../../cryptography/signature-scheme.js';
import { Ed25519PublicKey } from '../ed25519/publickey.js';

/**
 * An Ed25519Legacy public key
 */
export class Ed25519LegacyPublicKey extends Ed25519PublicKey {
	/**
	 * Return the Sui address associated with this Ed25519Legacy public key
	 */
	flag(): number {
		return SIGNATURE_SCHEME_TO_FLAG['ED25519Legacy'];
	}

	/**
	 * Create a new Ed25519 Legacy public key instance
	 * from a provided {@link Ed25519PublicKey}.
	 *
	 * @param ed25519keypair Ed25519 keypair
	 */
	static fromEd25519PublicKey(ed25519publickey: Ed25519PublicKey): Ed25519LegacyPublicKey {
		return new Ed25519LegacyPublicKey(ed25519publickey.toRawBytes());
	}
}
