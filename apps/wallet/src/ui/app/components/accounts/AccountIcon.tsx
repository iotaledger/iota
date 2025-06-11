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
        return <Ledger className="text-neutral-10 dark:text-neutral-92 h-5 w-5" />;
    }
    return <IotaLogoMark className="text-neutral-10 dark:text-neutral-92 h-5 w-5" />;
}
