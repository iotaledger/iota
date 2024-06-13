// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { BalanceChange } from '@iota/core';
import { useMemo } from 'react';
import { BalanceChangeEntry } from './';
import { getRecognizedUnRecognizedTokenChanges } from '@iota/core';

export default function BalanceChangeEntries({ changes }: { changes: BalanceChange[] }) {
    const { recognizedTokenChanges, unRecognizedTokenChanges } = useMemo(
        () => getRecognizedUnRecognizedTokenChanges(changes),
        [changes],
    );

    return (
        <div>
            {[...recognizedTokenChanges, ...unRecognizedTokenChanges].map((change) => (
                <BalanceChangeEntry change={change} key={change.coinType + change.amount} />
            ))}
        </div>
    );
}
