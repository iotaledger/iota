// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { getAllNetworks } from '@mysten/sui.js/client';
import type { IdentifierString } from '@wallet-standard/core';

export const SUPPORTED_CHAINS = Object.values(getAllNetworks()).map((network) => network.chain);

/**
 * Utility that returns whether or not a chain identifier is a supported chain.
 * @param chain a chain identifier in the form of `${string}:{$string}`
 */
export function isSupportedChain(chain: IdentifierString): boolean {
    return SUPPORTED_CHAINS.includes(chain);
}
