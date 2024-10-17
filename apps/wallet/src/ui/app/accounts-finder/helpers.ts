// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CoinBalance } from '@iota/iota-sdk/client';

export const getEmptyBalance = (coinType: string): CoinBalance => ({
    coinType: coinType,
    coinObjectCount: 0,
    totalBalance: '0',
    lockedBalance: {},
});
