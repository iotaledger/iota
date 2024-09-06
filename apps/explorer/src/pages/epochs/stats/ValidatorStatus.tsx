// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IOTA_PRIMITIVES_COLOR_PALETTE, Panel, Title, Tooltip } from '@iota/apps-ui-kit';
import { getRefGasPrice } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Info } from '@iota/ui-icons';
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
            value: 107,
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
            value: 12,
            label: 'New',
            color: getHexColorWithOpacity(IOTA_PRIMITIVES_COLOR_PALETTE.primary[30], 0.6),
        },
        {
            value: 7,
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
                        {/* Replace with Display Stats */}
                        <div className="flex h-full w-full flex-col items-start justify-between rounded-xl bg-neutral-96 p-md">
                            <div className="flex flex-row gap-xxxs">
                                <div className="flex flex-col">
                                    <span className="text-label-sm text-neutral-10">
                                        Estimated Next Epoch
                                    </span>
                                    <span className="text-label-sm text-neutral-10">
                                        Reference Gas Price
                                    </span>
                                </div>
                                <Tooltip text="Example Message">
                                    <Info className="h-4 w-4 text-neutral-10/40" />
                                </Tooltip>
                            </div>
                            <div className="flex flex-row items-baseline gap-xxs">
                                <span className="text-title-md text-neutral-10">
                                    {nextRefGasPrice.toString()}
                                </span>
                                <span className="text-label-md text-neutral-10/40">nano</span>
                            </div>
                        </div>
                    </div>
                </div>
            </div>
        </Panel>
    );
}
