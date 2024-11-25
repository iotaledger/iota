// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { Ledger, IotaLogoMark } from '@iota/ui-icons';

interface AccountIconProps {
    account: SerializedUIAccount;
}

export function AccountIcon({ account }: AccountIconProps) {
    if (account.type === AccountType.LedgerDerived) {
        return <Ledger className="h-5 w-5" />;
    }
    return <IotaLogoMark className="h-5 w-5" />;
}
