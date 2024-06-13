// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange } from '@iota/core';
import { BalanceChangeEntries } from './';

type BalanceChangeByOwner = Record<string, BalanceChange[]>;
type BalanceChangeSummary = BalanceChangeByOwner | null;

export default function BalanceChanges({
    balanceChanges,
}: {
    balanceChanges?: BalanceChangeSummary;
}) {
    if (!balanceChanges) return null;
    return (
        <>
            {Object.entries(balanceChanges).map(([owner, changes]) => (
                <div key={owner}>
                    <div>
                        <BalanceChangeEntries changes={changes} />
                    </div>
                </div>
            ))}
        </>
    );
}
