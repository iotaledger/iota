// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import { LedgerLogo17, Iota } from '@iota/icons';

function IotaIcon() {
    return (
        <div className="bg-steel flex h-4 w-4 items-center justify-center rounded-full p-1 text-white">
            <Iota />
        </div>
    );
}

interface AccountIconProps {
    account: SerializedUIAccount;
}

export function AccountIcon({ account }: AccountIconProps) {
    if (account.type === AccountType.LedgerDerived) {
        return <LedgerLogo17 className="h-4 w-4" />;
    }
    return <IotaIcon />;
}
