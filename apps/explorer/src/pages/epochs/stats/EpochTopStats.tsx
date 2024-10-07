// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ProgressBar } from '~/components';
import { EpochStatsGrid } from './EpochStats';
import { LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { formatDate } from '@iota/core';
import { TokenStats } from './TokenStats';
import { getSupplyChangeAfterEpochEnd } from '~/lib';
import { useEpochProgress } from '../utils';
import type { EndOfEpochInfo } from '@iota/iota-sdk/src/client';

interface EpochProgressProps {
    start: number;
    end?: number;
    inProgress?: boolean;
    endOfEpochInfo?: EndOfEpochInfo | null;
}

export function EpochTopStats({
    start,
    end,
    inProgress,
    endOfEpochInfo,
}: EpochProgressProps): React.JSX.Element {
    const { progress, label } = useEpochProgress();
    const endTime = inProgress ? label : end ? formatDate(end) : undefined;

    return (
        <div className="flex w-full flex-col gap-md--rs">
            {inProgress ? <ProgressBar progress={progress || 0} /> : null}

            <EpochStatsGrid>
                <LabelText text={formatDate(start)} label="Start" />
                {endTime ? <LabelText text={endTime} label="End" /> : null}
                {endOfEpochInfo && (
                    <TokenStats
                        label="Supply Change"
                        size={LabelTextSize.Large}
                        amount={getSupplyChangeAfterEpochEnd(endOfEpochInfo)}
                        showSign
                    />
                )}
            </EpochStatsGrid>
        </div>
    );
}
