// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, DisplayStatsType } from '@iota/apps-ui-kit';
import { formatDate, useFormatCoin, useGetClockTimestamp } from '@iota/core';
import { LockLocked } from '@iota/apps-ui-icons';

interface VestingScheduleBoxProps {
    amount: bigint;
    expirationTimestampMs: number;
}

export function VestingScheduleBox({
    amount,
    expirationTimestampMs,
}: VestingScheduleBoxProps): React.JSX.Element {
    const [formattedAmountVested, amountVestedSymbol] = useFormatCoin({ balance: amount });
    const { data: clockTimestampMs } = useGetClockTimestamp();

    const isLocked = expirationTimestampMs > clockTimestampMs;
    const transactionDate = formatDate(Number(expirationTimestampMs), [
        'day',
        'month',
        'year',
        'hour',
        'minute',
    ]);
    return (
        <DisplayStats
            label={transactionDate}
            value={`${formattedAmountVested} ${amountVestedSymbol}`}
            type={isLocked ? DisplayStatsType.Default : DisplayStatsType.Secondary}
            icon={isLocked && <LockLocked className="h-4 w-4" />}
        />
    );
}
