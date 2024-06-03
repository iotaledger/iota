// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/MnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/SeedAccount';

function getKey(account: SerializedUIAccount): string {
    if (isMnemonicSerializedUiAccount(account)) return account.sourceID;
    if (isSeedSerializedUiAccount(account)) return account.sourceID;
    return account.type;
}

export const defaultSortOrder: AccountType[] = [
    'mnemonic-derived',
    'seed-derived',
    'imported',
    'ledger',
];

export function groupByType(accounts: SerializedUIAccount[]) {
    return accounts.reduce(
        (acc, account) => {
            const byType = acc[account.type] || (acc[account.type] = {});
            const key = getKey(account);
            (byType[key] || (byType[key] = [])).push(account);
            return acc;
        },
        defaultSortOrder.reduce(
            (acc, type) => {
                acc[type] = {};
                return acc;
            },
            {} as Record<AccountType, Record<string, SerializedUIAccount[]>>,
        ),
    );
}
