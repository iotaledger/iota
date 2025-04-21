// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_PRIMITIVES_COLOR_PALETTE, Panel, Title } from '@iota/apps-ui-kit';
import { Theme, useGetLatestIotaSystemState, useTheme } from '@iota/core';
import { RingChart, RingChartLegend } from '~/components/ui';

export function ValidatorStatus(): JSX.Element | null {
    const { data } = useGetLatestIotaSystemState();
    const { theme } = useTheme();

    if (!data) return null;

    const nextEpoch = Number(data.epoch || 0) + 1;

    const getHexColorWithOpacity = (color: string, opacity: number) =>
        `${color}${Math.round(opacity * 255).toString(16)}`;

    const chartData = [
        {
            value: data.committeeMembers.length,
            label: 'Committee',
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
            value: data.activeValidators.length - data.committeeMembers.length,
            label: 'Active (not in committee)',
            gradient: {
                deg: 315,
                values: [
                    {
                        percent: 0,
                        color: getHexColorWithOpacity(
                            IOTA_PRIMITIVES_COLOR_PALETTE.primary[30],
                            0.6,
                        ),
                    },
                    {
                        percent: 100,
                        color: getHexColorWithOpacity(
                            IOTA_PRIMITIVES_COLOR_PALETTE.primary[30],
                            0.6,
                        ),
                    },
                ],
            },
        },
        {
            value: Number(data.pendingActiveValidatorsSize ?? 0),
            label: 'New',
            color: getHexColorWithOpacity(IOTA_PRIMITIVES_COLOR_PALETTE.primary[30], 0.3),
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
                </div>
            </div>
        </Panel>
    );
}
