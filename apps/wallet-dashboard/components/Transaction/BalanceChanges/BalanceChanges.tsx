// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange } from '@iota/core';
import { BalanceChangeEntries } from './';

interface BalanceChangesProps {
    balanceChanges?: Record<string, BalanceChange[]> | null;
}

export default function BalanceChanges({ balanceChanges }: BalanceChangesProps) {
    if (!balanceChanges) return null;
    return (
        <>
            {Object.entries(balanceChanges).map(([owner, balanceChanges]) => (
                <div key={owner}>
                    <BalanceChangeEntries balanceChanges={balanceChanges} />
                </div>
            ))}
        </>
    );
}
