// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { isLegacyAccount } from '_src/background/accounts/isLegacyAccount';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';

export function getKey(account: SerializedUIAccount): string {
    if (isMnemonicSerializedUiAccount(account)) return account.sourceID;
    if (isSeedSerializedUiAccount(account)) return account.sourceID;
    return account.type;
}

export const DEFAULT_SORT_ORDER: AccountType[] = [
    AccountType.MnemonicDerived,
    AccountType.SeedDerived,
    AccountType.PrivateKeyDerived,
    AccountType.LedgerDerived,
    AccountType.LegacyDerived,
];

export function groupByType(accounts: SerializedUIAccount[]) {
    return accounts.reduce(
        (acc, account) => {
            if (isLegacyAccount(account)) {
                const byType =
                    acc[AccountType.LegacyDerived] || (acc[AccountType.LegacyDerived] = {});
                const key = getKey(account);
                (byType[key] || (byType[key] = [])).push(account);
                return acc;
            } else {
                const byType = acc[account.type] || (acc[account.type] = {});
                const key = getKey(account);
                (byType[key] || (byType[key] = [])).push(account);
                return acc;
            }
        },
        DEFAULT_SORT_ORDER.reduce(
            (acc, type) => {
                acc[type] = {};
                return acc;
            },
            {} as Record<AccountType, Record<string, SerializedUIAccount[]>>,
        ),
    );
}
