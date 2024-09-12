// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats } from '@iota/apps-ui-kit';
import { formatDate } from '@iota/core';
interface TransactionDetailsProps {
    sender?: string;
    checkpoint?: string | null;
    executedEpoch?: string;
    timestamp?: string | null;
}

export function TransactionDetails({
    sender,
    checkpoint,
    executedEpoch,
    timestamp,
}: TransactionDetailsProps): JSX.Element {
    return (
        <div className="grid grid-cols-1 gap-sm md:grid-cols-4">
            {sender && (
                <DisplayStats
                    label="Sender"
                    value={sender}
                    valueLink={`/address/${sender}`}
                    isTruncated
                />
            )}
            {checkpoint && (
                <DisplayStats
                    label="Checkpoint"
                    value={Number(checkpoint).toLocaleString()}
                    valueLink={`/checkpoint/${checkpoint}`}
                />
            )}
            {executedEpoch && (
                <DisplayStats
                    label="Epoch"
                    value={executedEpoch}
                    valueLink={`/epoch/${executedEpoch}`}
                />
            )}

            {timestamp && <DisplayStats label="Date" value={formatDate(Number(timestamp))} />}
        </div>
    );
}
