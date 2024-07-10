// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { CoinBalance } from '@iota/iota.js/client';
import type { MakeDerivationOptions } from '_src/background/account-sources/bip44Path';
import { getAccountSourceByID } from '_src/background/account-sources';

export const getEmptyBalance = (coinType: string): CoinBalance => ({
    coinType: coinType,
    coinObjectCount: 0,
    totalBalance: '0',
    lockedBalance: {},
});

export async function getPublicKey(options: { sourceID: string } & MakeDerivationOptions) {
    const accountSource = await getAccountSourceByID(options.sourceID);

    if (!accountSource) {
        throw new Error('Could not find account source');
    }

    const pubKey = await accountSource.derivePubKey(options);
    return pubKey?.toIotaAddress();
}
