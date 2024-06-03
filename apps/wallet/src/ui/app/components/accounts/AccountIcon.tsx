// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { LedgerLogo17, Sui } from '@mysten/icons';

function SuiIcon() {
    return (
        <div className="flex h-4 w-4 items-center justify-center rounded-full bg-steel p-1 text-white">
            <Sui />
        </div>
    );
}

export function AccountIcon({ account }: { account: SerializedUIAccount }) {
    if (account.type === 'ledger') {
        return <LedgerLogo17 className="h-4 w-4" />;
    }
    return <SuiIcon />;
}
