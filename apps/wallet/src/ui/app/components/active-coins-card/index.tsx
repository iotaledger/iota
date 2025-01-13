// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useCoinsReFetchingConfig, useActiveAddress } from '_hooks';
import { Loading } from '_components';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Link } from 'react-router-dom';
import { filterAndSortTokenBalances, CoinItem } from '@iota/core';

interface ActiveCoinsCardProps {
    activeCoinType: string;
    showActiveCoin?: boolean;
}

export function ActiveCoinsCard({
    activeCoinType = IOTA_TYPE_ARG,
    showActiveCoin = true,
}: ActiveCoinsCardProps) {
    const selectedAddress = useActiveAddress();

    const { staleTime, refetchInterval } = useCoinsReFetchingConfig();
    const { data: coins, isPending } = useIotaClientQuery(
        'getAllBalances',
        { owner: selectedAddress! },
        {
            enabled: !!selectedAddress,
            refetchInterval,
            staleTime,
            select: filterAndSortTokenBalances,
        },
    );

    const activeCoin = coins?.find(({ coinType }) => coinType === activeCoinType);

    return (
        <Loading loading={isPending}>
            <div className="flex w-full">
                {showActiveCoin ? (
                    activeCoin && (
                        <Link
                            to={`/send/select?${new URLSearchParams({
                                type: activeCoin.coinType,
                            }).toString()}`}
                            className="border-gray-45 flex w-full items-center gap-2 overflow-hidden rounded-2lg border border-solid no-underline"
                        >
                            <CoinItem
                                coinType={activeCoin.coinType}
                                balance={BigInt(activeCoin.totalBalance)}
                            />
                        </Link>
                    )
                ) : (
                    <div className="flex w-full flex-col">
                        <div className="divide-gray-45 mt-2 flex flex-col items-center justify-between divide-x-0 divide-y divide-solid">
                            {coins?.map(({ coinType, totalBalance }) => (
                                <Link
                                    to={`/send?${new URLSearchParams({
                                        type: coinType,
                                    }).toString()}`}
                                    key={coinType}
                                    className="w-full no-underline"
                                >
                                    <CoinItem coinType={coinType} balance={BigInt(totalBalance)} />
                                </Link>
                            ))}
                        </div>
                    </div>
                )}
            </div>
        </Loading>
    );
}
