// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { LedgerLogo17, Iota } from '@iota/icons';

function IotaIcon() {
    return (
        <div className="flex h-4 w-4 items-center justify-center rounded-full bg-steel p-1 text-white">
            <Iota />
        </div>
    );
}

export function AccountIcon({ account }: { account: SerializedUIAccount }) {
    if (account.type === AccountType.Ledger) {
        return <LedgerLogo17 className="h-4 w-4" />;
    }
    return <IotaIcon />;
}
