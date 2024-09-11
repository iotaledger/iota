// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, IOTA_PRIMITIVES_COLOR_PALETTE, Panel, Title } from '@iota/apps-ui-kit';
import { getRefGasPrice } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';

import { RingChart, RingChartLegend } from '~/components/ui';

export function ValidatorStatus(): JSX.Element | null {
    const { data } = useIotaClientQuery('getLatestIotaSystemState');

    const nextRefGasPrice = useMemo(
        () => getRefGasPrice(data?.activeValidators),
        [data?.activeValidators],
    );

    if (!data) return null;

    const nextEpoch = Number(data.epoch || 0) + 1;

    const getHexColorWithOpacity = (color: string, opacity: number) =>
        `${color}${Math.round(opacity * 255).toString(16)}`;

    const chartData = [
        {
            value: data.activeValidators.length,
            label: 'Active',
            gradient: {
                deg: 315,
                values: [
                    { percent: 0, color: IOTA_PRIMITIVES_COLOR_PALETTE.primary[30] },
                    { percent: 100, color: IOTA_PRIMITIVES_COLOR_PALETTE.primary[30] },
                ],
            },
        },
        {
            value: Number(data.pendingActiveValidatorsSize ?? 0),
            label: 'New',
            color: getHexColorWithOpacity(IOTA_PRIMITIVES_COLOR_PALETTE.primary[30], 0.6),
        },
        {
            value: data.atRiskValidators.length,
            label: 'At Risk',
            color: IOTA_PRIMITIVES_COLOR_PALETTE.neutral[90],
        },
    ];

    return (
        <Panel>
            <div className="flex flex-col">
                <Title title={`Validators in Epoch ${nextEpoch}`} />
                <div className="flex flex-col items-center gap-sm--rs p-md--rs md:flex-row ">
                    <div className="flex w-1/2 flex-row gap-x-lg p-md">
                        <div className="min-h-[96px] min-w-[96px]">
                            <RingChart data={chartData} />
                        </div>

                        <RingChartLegend data={chartData} />
                    </div>

                    <div className="h-full w-1/2">
                        <DisplayStats
                            label="Estimated Next Epoch
                            Reference Gas Price"
                            value={nextRefGasPrice.toString()}
                            supportingLabel="nano"
                        />
                    </div>
                </div>
            </div>
        </Panel>
    );
}
