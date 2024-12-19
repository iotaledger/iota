// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useGetCurrentEpochStartTimestamp } from '@/hooks';
import { DisplayStats, DisplayStatsType } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { LockLocked } from '@iota/ui-icons';

interface VestingScheduleBoxProps {
    amount: bigint;
    expirationTimestampMs: number;
}

export function VestingScheduleBox({
    amount,
    expirationTimestampMs,
}: VestingScheduleBoxProps): React.JSX.Element {
    const [formattedAmountVested, amountVestedSymbol] = useFormatCoin(amount, IOTA_TYPE_ARG);
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();

    const isLocked = expirationTimestampMs > Number(currentEpochMs);
    return (
        <DisplayStats
            label={new Date(expirationTimestampMs).toLocaleDateString()}
            value={`${formattedAmountVested} ${amountVestedSymbol}`}
            type={isLocked ? DisplayStatsType.Default : DisplayStatsType.Secondary}
            icon={isLocked && <LockLocked className="h-4 w-4" />}
        />
    );
}
