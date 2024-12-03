// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, DisplayStatsType } from '@iota/apps-ui-kit';
import { useFormatCoin } from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

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
            label={
                <>
                    <span>{new Date(expirationTimestampMs).toLocaleString()}</span>
                    {/* Show lock icon if it's a future date */}
                    {isLocked && <span>Look</span>}
                </>
            }
            value={`${formattedAmountVested} ${amountVestedSymbol}`}
            type={isLocked ? DisplayStatsType.Default : DisplayStatsType.Secondary}
        />
    );
}
