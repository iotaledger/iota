// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange } from '@iota/core';
import { BalanceChangeEntries } from './';

interface BalanceChangesProps {
    balanceChanges: Record<string, BalanceChange[]>;
}

export default function BalanceChanges({ balanceChanges }: BalanceChangesProps) {
    return (
        <>
            {Object.entries(balanceChanges).map(([owner, balanceChanges], index) => (
                <BalanceChangeEntries balanceChanges={balanceChanges} key={owner + index} />
            ))}
        </>
    );
}
