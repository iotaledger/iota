// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ProgressBar } from '~/components';
import { EpochStatsGrid } from './EpochStats';
import { LabelText, LabelTextSize } from '@iota/apps-ui-kit';
import { Feature, formatDate, useFeatureEnabledByNetwork } from '@iota/core';
import { TokenStats } from './TokenStats';
import { getSupplyChangeAfterEpochEnd } from '~/lib';
import { useEpochProgress } from '../utils';
import type { Network, EndOfEpochInfo } from '@iota/iota-sdk/client';
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

    const isBurntAndMintedTokensInEndedEpochsFeatureEnabled = useFeatureEnabledByNetwork(
        Feature.BurntAndMintedTokensInEndedEpochs,
        network as Network,
    );

    return (
        <div className="flex w-full flex-col gap-md--rs">
            {inProgress ? <ProgressBar progress={progress || 0} /> : null}

            <EpochStatsGrid>
                <LabelText text={formatDate(start)} label="Start" />
                {endTime ? <LabelText text={endTime} label="End" /> : null}
                {endOfEpochInfo && (
                    <>
                        {isBurntAndMintedTokensInEndedEpochsFeatureEnabled && (
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
