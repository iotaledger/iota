// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { Ledger, IotaLogoMark } from '@iota/apps-ui-icons';

interface AccountIconProps {
    account: SerializedUIAccount;
}

export function AccountIcon({ account }: AccountIconProps) {
    if (account.type === AccountType.LedgerDerived) {
        return <Ledger className="h-5 w-5 text-neutral-10 dark:text-neutral-92" />;
    }
    return <IotaLogoMark className="h-5 w-5 text-neutral-10 dark:text-neutral-92" />;
}
