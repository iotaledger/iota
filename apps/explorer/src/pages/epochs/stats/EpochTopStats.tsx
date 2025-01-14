// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ProgressBar } from '~/components';
import { EpochStatsGrid } from './EpochStats';
import { LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { Feature, formatDate } from '@iota/core';
import { TokenStats } from './TokenStats';
import { getSupplyChangeAfterEpochEnd } from '~/lib';
import { useEpochProgress } from '../utils';
import type { Network, EndOfEpochInfo } from '@iota/iota-sdk/client';
import { useFeature } from '@growthbook/growthbook-react';
import { useNetworkContext } from '~/contexts';

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
    const [network] = useNetworkContext();

    const endTime = inProgress ? label : end ? formatDate(end) : undefined;

    const featureBurntAndMintedTokensInEndedEpochsEnabled = useFeature<{
        [key in Network]: boolean;
    }>(Feature.BurntAndMintedTokensInEndedEpochs).value;

    const isFeatureEnabledForCurrentNetwork =
        featureBurntAndMintedTokensInEndedEpochsEnabled?.[
            network as keyof typeof featureBurntAndMintedTokensInEndedEpochsEnabled
        ];

    return (
        <div className="flex w-full flex-col gap-md--rs">
            {inProgress ? <ProgressBar progress={progress || 0} /> : null}

            <EpochStatsGrid>
                <LabelText text={formatDate(start)} label="Start" />
                {endTime ? <LabelText text={endTime} label="End" /> : null}
                {endOfEpochInfo && (
                    <>
                        {isFeatureEnabledForCurrentNetwork && (
                            <>
                                <TokenStats
                                    label="Burnt Tokens"
                                    size={LabelTextSize.Large}
                                    amount={BigInt(endOfEpochInfo?.burntTokensAmount)}
                                    showSign
                                />
                                <TokenStats
                                    label="Minted Tokens"
                                    size={LabelTextSize.Large}
                                    amount={BigInt(endOfEpochInfo?.mintedTokensAmount)}
                                    showSign
                                />
                            </>
                        )}
                        <TokenStats
                            label="Supply Change"
                            size={LabelTextSize.Large}
                            amount={getSupplyChangeAfterEpochEnd(endOfEpochInfo)}
                            showSign
                        />
                    </>
                )}
            </EpochStatsGrid>
        </div>
    );
}
