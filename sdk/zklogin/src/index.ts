// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import '@iota/iota-sdk/zklogin';

import type { ComputeZkLoginAddressOptions } from '@iota/iota-sdk/zklogin';
import {
	computeZkLoginAddress as iotaComputeZkLoginAddress,
	jwtToAddress as iotaJwtToAddress,
} from '@iota/iota-sdk/zklogin';

export type { ComputeZkLoginAddressOptions } from '@iota/iota-sdk/zklogin';

export {
	/** @deprecated, use `import { genAddressSeed } from '@iota/iota-sdk/zklogin';` instead */
	genAddressSeed,
	/** @deprecated, use `import { generateNonce } from '@iota/iota-sdk/zklogin';` instead */
	generateNonce,
	/** @deprecated, use `import { generateRandomness } from '@iota/iota-sdk/zklogin';` instead */
	generateRandomness,
	/** @deprecated, use `import { getExtendedEphemeralPublicKey } from '@iota/iota-sdk/zklogin';` instead */
	getExtendedEphemeralPublicKey,
	/** @deprecated, use `import { getZkLoginSignature } from '@iota/iota-sdk/zklogin';` instead */
	getZkLoginSignature,
	/** @deprecated, use `import { hashASCIIStrToField } from '@iota/iota-sdk/zklogin';` instead */
	hashASCIIStrToField,
	/** @deprecated, use `import { poseidonHash } from '@iota/iota-sdk/zklogin';` instead */
	poseidonHash,
} from '@iota/iota-sdk/zklogin';

/** @deprecated, use `import { parseZkLoginSignature } from '@iota/iota-sdk/zklogin';` instead */
export function computeZkLoginAddress(options: ComputeZkLoginAddressOptions) {
	return iotaComputeZkLoginAddress({
		...options,
		legacyAddress: true,
	});
}

/** @deprecated, use `import { jwtToAddress } from '@iota/iota-sdk/zklogin';` instead */
export function jwtToAddress(jwt: string, userSalt: string | bigint, legacyAddress = true) {
	return iotaJwtToAddress(jwt, userSalt, legacyAddress);
}
