// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export { computeZkLoginAddress, jwtToAddress } from './address.js';
export type { ComputeZkLoginAddressOptions } from './address.js';

export { getZkLoginSignature } from '@iota/iota.js/zklogin';

export { poseidonHash } from './poseidon.js';

export { generateNonce, generateRandomness } from './nonce.js';

export { hashASCIIStrToField, genAddressSeed, getExtendedEphemeralPublicKey } from './utils.js';
