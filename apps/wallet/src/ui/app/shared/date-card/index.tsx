// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import { formatDate } from '_helpers';

interface DateCardProps {
    timestamp: number;
    size: 'sm' | 'md';
}

export function DateCard({ timestamp, size }: DateCardProps) {
    const txnDate = formatDate(timestamp, ['month', 'day', 'hour', 'minute']);

    return (
        <Text
            color="steel-dark"
            weight={size === 'sm' ? 'medium' : 'normal'}
            variant={size === 'sm' ? 'subtitleSmallExtra' : 'pBodySmall'}
        >
            {txnDate}
        </Text>
    );
}
