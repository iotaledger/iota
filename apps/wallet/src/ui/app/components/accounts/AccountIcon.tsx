// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { Ledger, IotaLogoMark, Keystone, Passkey } from '@iota/apps-ui-icons';

interface AccountIconProps {
    account: SerializedUIAccount;
}

export function AccountIcon({ account }: AccountIconProps) {
    if (account.type === AccountType.LedgerDerived) {
        return <Ledger className="h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-92" />;
    }
    if (account.type === AccountType.KeystoneDerived) {
        return <Keystone className="h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-92" />;
    }
    if (account.type === AccountType.PasskeyDerived) {
        return <Passkey className="h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-92" />;
    }
    return <IotaLogoMark className="h-5 w-5 text-iota-neutral-10 dark:text-iota-neutral-92" />;
}
