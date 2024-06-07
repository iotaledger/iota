// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { WalletAccount } from '@iota/wallet-standard';

export function getSelectedAccount(
    connectedAccounts: readonly WalletAccount[],
    accountAddress?: string,
) {
    if (connectedAccounts.length === 0) {
        return null;
    }

    if (accountAddress) {
        const selectedAccount = connectedAccounts.find(
            (account) => account.address === accountAddress,
        );
        return selectedAccount ?? connectedAccounts[0];
    }

    return connectedAccounts[0];
}
