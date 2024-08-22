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
            <Title title="Network Activity" size={TitleSize.Small} />
            <div className="mt-sm flex flex-col gap-6 px-md py-sm--rs">
                <div className="flex">
                    <div className="min-w-[164px]">
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
                <Divider />
                <div className="flex">
                    <div className="min-w-[164px]">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Total Packages"
                            text={networkMetrics?.totalPackages ?? '-'}
                            showSupportingLabel={false}
                        />
                    </div>

                    <LabelText
                        size={LabelTextSize.Large}
                        label="Objects"
                        text={networkMetrics?.totalObjects ?? '-'}
                        showSupportingLabel={false}
                    />
                </div>
                <div className="flex">
                    <div className="min-w-[164px]">
                        <LabelText
                            size={LabelTextSize.Large}
                            label="Reference Gas Price"
                            text={gasPriceFormatted ?? '-'}
                            showSupportingLabel={gasPriceFormatted !== null}
                            supportingLabel="IOTA"
                        />
                    </div>
                    <LabelText
                        size={LabelTextSize.Large}
                        label="Total Supply"
                        text={totalSupplyFormatted ?? '-'}
                        showSupportingLabel={totalSupplyFormatted !== null}
                        supportingLabel="IOTA"
                    />
                </div>
            </div>
        </Panel>
    );
}
