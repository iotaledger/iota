// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { isKeystoneAccountSerializedUI } from '_src/background/accounts/keystoneAccount';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';

export function getKey(account: SerializedUIAccount): string {
    if (isMnemonicSerializedUiAccount(account)) return account.sourceID;
    if (isSeedSerializedUiAccount(account)) return account.sourceID;
    if (isKeystoneAccountSerializedUI(account)) return account.sourceID;
    if (isLedgerAccountSerializedUI(account) && account.mainPublicKey) return account.mainPublicKey;
    return account.type;
}

export function getSourceId(account: SerializedUIAccount): string {
    if (isMnemonicSerializedUiAccount(account)) return account.sourceID;
    if (isSeedSerializedUiAccount(account)) return account.sourceID;
    if (isKeystoneAccountSerializedUI(account)) return account.sourceID;
    return account.type;
}

export const DEFAULT_SORT_ORDER: AccountType[] = [
    AccountType.MnemonicDerived,
    AccountType.SeedDerived,
    AccountType.PrivateKeyDerived,
    AccountType.LedgerDerived,
    AccountType.PasskeyDerived,
    AccountType.KeystoneDerived,
];

export function groupByType(accounts: SerializedUIAccount[]) {
    return accounts.reduce(
        (acc, account) => {
            const byType = acc[account.type] || (acc[account.type] = {});
            const key = getKey(account);
            const sourceId = getSourceId(account);
            (byType[key] || (byType[key] = { sourceId, accounts: [] })).accounts.push(account);
            return acc;
        },
        DEFAULT_SORT_ORDER.reduce(
            (acc, type) => {
                acc[type] = {};
                return acc;
            },
            {} as Record<
                AccountType,
                Record<string, { sourceId: string; accounts: SerializedUIAccount[] }>
            >,
        ),
    );
}

/**
 * Determines if the provided accounts array represents the first account being added.
 * Returns true if there are no existing accounts.
 */
export function isFirstAccount(accounts?: SerializedUIAccount[]): boolean {
    return !accounts?.length;
}
