// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange } from '@iota/core';
import { useMemo } from 'react';
import { BalanceChangeEntry } from './';
import { getRecognizedUnRecognizedTokenChanges } from '@iota/core';

interface BalanceChangeEntriesProps {
    balanceChanges: BalanceChange[];
}

export default function BalanceChangeEntries({
    balanceChanges: changes,
}: BalanceChangeEntriesProps) {
    const { recognizedTokenChanges, unRecognizedTokenChanges } = useMemo(
        () => getRecognizedUnRecognizedTokenChanges(changes),
        [changes],
    );

    return (
        <div>
            {[...recognizedTokenChanges, ...unRecognizedTokenChanges].map(
                (balanceChange, index) => (
                    <BalanceChangeEntry balanceChange={balanceChange} key={index} />
                ),
            )}
        </div>
    );
}
