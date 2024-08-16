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

export function TokenStakingOverview({
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

    const isLoading = isPending || queryResultStake.isPending;

    return (
        <Card type={CardType.Filled} onClick={handleOnClick} isDisabled={disabled}>
            <CardImage shape={ImageShape.SquareRounded}>
                <Stake className="h-5 w-5 text-primary-20" />
            </CardImage>
            <CardBody
                title={
                    isLoading
                        ? '--'
                        : totalDelegatedStake
                          ? `${formattedDelegatedStake} ${symbol}`
                          : 'Start Staking'
                }
                subtitle={isLoading ? '--' : totalDelegatedStake ? 'Current Stake' : 'Earn Rewards'}
            />
            <CardAction type={CardActionType.Link} onClick={handleOnClick} />
        </Card>
    );
}
