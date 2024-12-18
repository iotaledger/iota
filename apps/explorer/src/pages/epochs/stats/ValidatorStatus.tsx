// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DisplayStats, IOTA_PRIMITIVES_COLOR_PALETTE, Panel, Title } from '@iota/apps-ui-kit';
import { getRefGasPrice, useTheme, Theme } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { RingChart, RingChartLegend } from '~/components/ui';

export function ValidatorStatus(): JSX.Element | null {
    const { data } = useIotaClientQuery('getLatestIotaSystemState');
    const { theme } = useTheme();

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
                    {
                        percent: 0,
                        color: IOTA_PRIMITIVES_COLOR_PALETTE.primary[30],
                    },
                    {
                        percent: 100,
                        color: IOTA_PRIMITIVES_COLOR_PALETTE.primary[30],
                    },
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
            color:
                theme === Theme.Dark
                    ? IOTA_PRIMITIVES_COLOR_PALETTE.neutral[20]
                    : IOTA_PRIMITIVES_COLOR_PALETTE.neutral[90],
        },
    ];

    return (
        <Panel>
            <div className="flex flex-col">
                <Title title={`Validators in Epoch ${nextEpoch}`} />
                <div className="flex flex-col items-start justify-center gap-x-xl gap-y-sm p-md--rs md:flex-row md:items-center md:justify-between md:gap-sm--rs">
                    <div className="flex w-auto flex-row gap-x-md p-md md:max-w-[50%]">
                        <div className="h-[92px] w-[92px]">
                            <RingChart data={chartData} width={4} />
                        </div>
                        <div className="flex flex-col items-center justify-center gap-xs lg:items-start">
                            <RingChartLegend data={chartData} />
                        </div>
                    </div>

                    <div className="h-full w-full max-w-[250px] sm:w-1/2 md:w-auto lg:w-1/2">
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
