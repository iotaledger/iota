// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext } from 'react';
import { type Network } from '@iota/iota-sdk/client';

export const NetworkContext = createContext<
    [Network | string, (network: Network | string) => void]
>(['', () => null]);

export function useNetworkContext(): [Network | string, (network: Network | string) => void] {
    return useContext(NetworkContext);
}
