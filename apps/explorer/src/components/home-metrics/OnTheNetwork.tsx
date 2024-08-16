// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatBalance } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Heading } from '@iota/ui';

import { Card, Divider } from '~/components/ui';
import { useGetNetworkMetrics } from '~/hooks/useGetNetworkMetrics';
import { FormattedStatsAmount, StatsWrapper } from './FormattedStatsAmount';
import { IOTA_DECIMALS, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export function OnTheNetwork(): JSX.Element {
    const { data: networkMetrics } = useGetNetworkMetrics();
    const { data: referenceGasPrice } = useIotaClientQuery('getReferenceGasPrice');
    const { data: totalSupply } = useIotaClientQuery('getTotalSupply', {
        coinType: IOTA_TYPE_ARG,
    });
    const gasPriceFormatted =
        typeof referenceGasPrice === 'bigint'
            ? formatBalance(referenceGasPrice, 0, CoinFormat.FULL)
            : null;
    const totalSupplyFormatted = totalSupply?.value
        ? formatBalance(totalSupply.value, IOTA_DECIMALS, CoinFormat.FULL)
        : null;
    return (
        <Card bg="white/80" spacing="lg" height="full">
            <div className="flex flex-col gap-4">
                <Heading variant="heading4/semibold" color="steel-darker">
                    Network Activity
                </Heading>
                <div className="flex gap-6">
                    <FormattedStatsAmount
                        label="TPS now"
                        amount={
                            networkMetrics?.currentTps
                                ? Math.floor(networkMetrics.currentTps)
                                : undefined
                        }
                        size="md"
                    />
                    <FormattedStatsAmount
                        label="Peak 30d TPS"
                        tooltip="Peak TPS in the past 30 days excluding this epoch"
                        amount={
                            networkMetrics?.tps30Days
                                ? Math.floor(networkMetrics?.tps30Days)
                                : undefined
                        }
                        size="md"
                    />
                </div>
                <Divider color="hero/10" />

                <StatsWrapper
                    orientation="horizontal"
                    label="Reference Gas Price"
                    tooltip="The reference gas price of the current epoch"
                    postfix={gasPriceFormatted !== null ? 'nano' : null}
                    size="sm"
                >
                    {gasPriceFormatted}
                </StatsWrapper>

                <Divider color="hero/10" />

                <div className="flex flex-1 flex-col gap-2">
                    <FormattedStatsAmount
                        orientation="horizontal"
                        label="Total Packages"
                        amount={networkMetrics?.totalPackages}
                        size="sm"
                    />
                    <FormattedStatsAmount
                        orientation="horizontal"
                        label="Objects"
                        amount={networkMetrics?.totalObjects}
                        size="sm"
                    />
                    <StatsWrapper
                        orientation="horizontal"
                        label="Total Supply"
                        size="sm"
                        postfix={totalSupplyFormatted !== null ? 'IOTA' : null}
                    >
                        {totalSupplyFormatted}
                    </StatsWrapper>
                </div>
            </div>
        </Card>
    );
}
