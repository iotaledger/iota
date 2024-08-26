// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, formatBalance } from '@iota/core';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { Panel, Divider, LabelText, LabelTextSize, Title, TitleSize } from '@iota/apps-ui-kit';

import { useGetNetworkMetrics } from '~/hooks/useGetNetworkMetrics';
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
        ? formatBalance(totalSupply.value, IOTA_DECIMALS, CoinFormat.ROUNDED)
        : null;
    return (
        <Panel>
            <Title title="Network Activity" size={TitleSize.Medium} />
            <div className="gap-md p-md--rs flex flex-col">
                <div className="gap-md flex">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="TPS Now"
                            text={
                                networkMetrics?.currentTps
                                    ? Math.floor(networkMetrics.currentTps).toString()
                                    : '-'
                            }
                            showSupportingLabel={false}
                        />
                    </div>

                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Peak 30d TPS"
                            text={
                                networkMetrics?.tps30Days
                                    ? Math.floor(networkMetrics?.tps30Days).toString()
                                    : '-'
                            }
                            showSupportingLabel={false}
                        />
                    </div>
                </div>

                <Divider />

                <div className="gap-x-md flex">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Packages"
                            text={networkMetrics?.totalPackages ?? '-'}
                            showSupportingLabel={false}
                        />
                    </div>
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Objects"
                            text={networkMetrics?.totalObjects ?? '-'}
                            showSupportingLabel={false}
                        />
                    </div>
                </div>

                <div className="gap-md flex">
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Reference Gas Price"
                            text={gasPriceFormatted ?? '-'}
                            showSupportingLabel={gasPriceFormatted !== null}
                            supportingLabel="IOTA"
                        />
                    </div>
                    <div className="flex-1">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Supply"
                            text={totalSupplyFormatted ?? '-'}
                            showSupportingLabel={totalSupplyFormatted !== null}
                            supportingLabel="IOTA"
                        />
                    </div>
                </div>
            </div>
        </Panel>
    );
}
