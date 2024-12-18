// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext } from 'react';

import type { WalletStore } from '../walletStore.js';

export const WalletContext = createContext<WalletStore | null>(null);
