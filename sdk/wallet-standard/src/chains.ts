// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IdentifierString } from '@wallet-standard/core';

/** Iota Devnet */
export const IOTA_DEVNET_CHAIN = 'iota:devnet';

/** Iota Testnet */
export const IOTA_TESTNET_CHAIN = 'iota:testnet';

/** Iota Localnet */
export const IOTA_LOCALNET_CHAIN = 'iota:localnet';

/** Iota Mainnet */
export const IOTA_MAINNET_CHAIN = 'iota:mainnet';

export const IOTA_CHAINS = [
    IOTA_DEVNET_CHAIN,
    IOTA_TESTNET_CHAIN,
    IOTA_LOCALNET_CHAIN,
    IOTA_MAINNET_CHAIN,
] as const;

export type IotaChain =
    | typeof IOTA_DEVNET_CHAIN
    | typeof IOTA_TESTNET_CHAIN
    | typeof IOTA_LOCALNET_CHAIN
    | typeof IOTA_MAINNET_CHAIN;

/**
 * Utility that returns whether or not a chain identifier is a valid Iota chain.
 * @param chain a chain identifier in the form of `${string}:{$string}`
 */
export function isIotaChain(chain: IdentifierString): chain is IotaChain {
    return IOTA_CHAINS.includes(chain as IotaChain);
}
