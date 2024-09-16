// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType } from '_src/background/accounts/Account';

import { BadgeLabel } from './BadgeLabel';

interface AccountBadgeProps {
    accountType: AccountType;
}

const TYPE_TO_TEXT: Record<AccountType, string | null> = {
    [AccountType.LedgerDerived]: 'Ledger',
    [AccountType.PrivateKeyDerived]: 'Private Key',
    [AccountType.MnemonicDerived]: null,
    [AccountType.SeedDerived]: null,
};

export function AccountBadge({ accountType }: AccountBadgeProps) {
    const badgeText = TYPE_TO_TEXT[accountType];

    if (!badgeText) return null;

    return <BadgeLabel label={badgeText} />;
}
