// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli } from '_src/shared/analytics/ampli';
import {
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    Card,
    CardAction,
    CardActionType,
    CardBody,
    CardImage,
    CardType,
    ImageShape,
} from '@iota/apps-ui-kit';
import { useNavigate } from 'react-router-dom';
import { Stake } from '@iota/ui-icons';

export function TokenIconLink({
    accountAddress,
    disabled,
}: {
    accountAddress: string;
    disabled: boolean;
}) {
    const navigate = useNavigate();
    const { data: delegatedStake, isPending } = useGetDelegatedStake({
        address: accountAddress,
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    // Total active stake for all delegations
    const delegatedStakes = delegatedStake ? formatDelegatedStake(delegatedStake) : [];
    const totalDelegatedStake = useTotalDelegatedStake(delegatedStakes);
    const [formattedDelegatedStake, symbol, queryResultStake] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );

    function handleOnClick() {
        navigate('/stake');
        ampli.clickedStakeIota({
            isCurrentlyStaking: totalDelegatedStake > 0,
            sourceFlow: 'Home page',
        });
    }

    return (
        <Card type={CardType.Filled} onClick={handleOnClick} isDisabled={disabled}>
            <CardImage shape={ImageShape.SquareRounded}>
                {isPending || queryResultStake.isPending ? (
                    <svg
                        className="-ml-1 mr-3 h-5 w-5 animate-spin text-primary-20"
                        xmlns="http://www.w3.org/2000/svg"
                        fill="none"
                        viewBox="0 0 24 24"
                    >
                        <circle
                            className="opacity-25"
                            cx="12"
                            cy="12"
                            r="10"
                            stroke="currentColor"
                            stroke-width="4"
                        ></circle>
                        <path
                            className="opacity-75"
                            fill="currentColor"
                            d="M4 12a8 8 0 018-8V0C5.373 0 0 5.373 0 12h4zm2 5.291A7.962 7.962 0 014 12H0c0 3.042 1.135 5.824 3 7.938l3-2.647z"
                        ></path>
                    </svg>
                ) : (
                    <Stake className="h-5 w-5 text-primary-20" />
                )}
            </CardImage>
            <CardBody
                title={
                    totalDelegatedStake ? `${formattedDelegatedStake} ${symbol}` : 'Start Staking'
                }
                subtitle={formattedDelegatedStake ? 'Current Stake' : 'Earn Rewards'}
            />
            <CardAction type={CardActionType.Link} onClick={handleOnClick} />
        </Card>
    );
}
