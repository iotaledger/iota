// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats } from '@iota/apps-ui-kit';
import { formatDate } from '@iota/core';
import { AddressLink, CheckpointSequenceLink, EpochLink } from '~/components';

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
                    value={<AddressLink address={sender}>{truncate(sender)}</AddressLink>}
                />
            )}
            {checkpoint && (
                <DisplayStats
                    label="Checkpoint"
                    value={
                        <CheckpointSequenceLink sequence={checkpoint}>
                            {Number(checkpoint).toLocaleString()}
                        </CheckpointSequenceLink>
                    }
                />
            )}
            {executedEpoch && (
                <DisplayStats
                    label="Epoch"
                    value={<EpochLink epoch={executedEpoch}>{executedEpoch}</EpochLink>}
                />
            )}

            {timestamp && <DisplayStats label="Date" value={formatDate(Number(timestamp))} />}
        </div>
    );
}
