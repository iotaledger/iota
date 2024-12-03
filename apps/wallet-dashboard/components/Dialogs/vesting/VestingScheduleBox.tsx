// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, DisplayStatsType } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { LockLocked } from '@iota/ui-icons';

interface VestingScheduleBoxProps {
    amount: number;
    expirationTimestampMs: number;
}

export function VestingScheduleBox({
    amount,
    expirationTimestampMs,
}: VestingScheduleBoxProps): React.JSX.Element {
    const [formattedAmountVested, amountVestedSymbol] = useFormatCoin(amount, IOTA_TYPE_ARG);
    const isLocked = expirationTimestampMs > Date.now();
    return (
        <DisplayStats
            label={new Date(expirationTimestampMs).toLocaleString()}
            value={`${formattedAmountVested} ${amountVestedSymbol}`}
            type={isLocked ? DisplayStatsType.Default : DisplayStatsType.Secondary}
            icon={isLocked && <LockLocked className="h-4 w-4" />}
        />
    );
}
