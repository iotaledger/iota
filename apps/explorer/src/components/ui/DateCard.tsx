// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { formatDate } from '@iota/core';
import { Text } from '@iota/ui';

export interface DateCardProps {
    date: Date | number;
}

// TODO - add format options
export function DateCard({ date }: DateCardProps): JSX.Element | null {
    const dateStr = formatDate(date, ['month', 'day', 'year', 'hour', 'minute']);

    if (!dateStr) {
        return null;
    }

    return (
        <Text variant="bodySmall/semibold" color="steel-dark">
            <time dateTime={new Date(date).toISOString()}>{dateStr}</time>
        </Text>
    );
}
