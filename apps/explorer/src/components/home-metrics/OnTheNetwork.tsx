// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatBalance } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Divider, LabelText, LabelTextSize, Panel, Title, TitleSize } from '@iota/apps-ui-kit';

import { useGetNetworkMetrics } from '~/hooks/useGetNetworkMetrics';
import { IOTA_DECIMALS, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

const FALLBACK = '--';

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
        ? formatBalance(totalSupply.value, IOTA_DECIMALS, CoinFormat.ROUNDED)
        : null;

    const currentTpsFormatted = networkMetrics?.currentTps
        ? formatBalance(Math.floor(networkMetrics.currentTps), 0, CoinFormat.ROUNDED)
        : FALLBACK;

    const tps30DaysFormatted = networkMetrics?.tps30Days
        ? formatBalance(Math.floor(networkMetrics.tps30Days), 0, CoinFormat.ROUNDED)
        : FALLBACK;

    const totalPackagesFormatted = networkMetrics?.totalPackages
        ? formatBalance(networkMetrics.totalPackages, 0, CoinFormat.ROUNDED)
        : FALLBACK;

    const totalObjectsFormatted = networkMetrics?.totalObjects
        ? formatBalance(networkMetrics.totalObjects, 0, CoinFormat.ROUNDED)
        : FALLBACK;

    return (
        <Panel>
            <Title title="Network Activity" size={TitleSize.Medium} />
            <div className="flex flex-col gap-md p-md--rs">
                <div className="flex gap-md">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="TPS Now"
                            text={currentTpsFormatted}
                        />
                    </div>

                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Peak 30d TPS"
                            text={tps30DaysFormatted}
                        />
                    </div>
                </div>

                <Divider />

                <div className="flex gap-x-md">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Packages"
                            text={totalPackagesFormatted}
                        />
                    </div>
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Objects"
                            text={totalObjectsFormatted}
                        />
                    </div>
                </div>

                <div className="flex gap-md">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Reference Gas Price"
                            text={gasPriceFormatted ?? '-'}
                            supportingLabel={gasPriceFormatted !== null ? 'IOTA' : undefined}
                        />
                    </div>
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Supply"
                            text={totalSupplyFormatted ?? '-'}
                            supportingLabel={totalSupplyFormatted !== null ? 'IOTA' : undefined}
                        />
                    </div>
                </div>
            </div>
        </Panel>
    );
}
