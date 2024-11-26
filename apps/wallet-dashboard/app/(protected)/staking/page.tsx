// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { AmountBox, Box, StakeCard, StakeDialog, Button } from '@/components';
import { useStakeDialog } from '@/components/Dialogs/Staking/hooks/useStakeDialog';

import {
    ExtendedDelegatedStake,
    formatDelegatedStake,
    useFormatCoin,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
} from '@iota/core';
import { useCurrentAccount } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { StakeDialogView } from '@/components/Dialogs/Staking/StakeDialog';

function StakingDashboardPage(): JSX.Element {
    const account = useCurrentAccount();

    const {
        isDialogStakeOpen,
        stakeDialogView,
        setStakeDialogView,
        selectedStake,
        setSelectedStake,
        selectedValidator,
        setSelectedValidator,
        handleCloseStakeDialog,
        handleNewStake,
    } = useStakeDialog();

    const { data: delegatedStakeData } = useGetDelegatedStake({
        address: account?.address || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const extendedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(extendedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(extendedStakes);
    const [formattedDelegatedStake, stakeSymbol, stakeResult] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );
    const [formattedDelegatedRewards, rewardsSymbol, rewardsResult] = useFormatCoin(
        totalDelegatedRewards,
        IOTA_TYPE_ARG,
    );

    const viewStakeDetails = (extendedStake: ExtendedDelegatedStake) => {
        setStakeDialogView(StakeDialogView.Details);
        setSelectedStake(extendedStake);
    };

    return (
        <>
            <div className="flex flex-col items-center justify-center gap-4 pt-12">
                <AmountBox
                    title="Currently staked"
                    amount={
                        stakeResult.isPending ? '-' : `${formattedDelegatedStake} ${stakeSymbol}`
                    }
                />
                <AmountBox
                    title="Earned"
                    amount={`${
                        rewardsResult.isPending ? '-' : formattedDelegatedRewards
                    } ${rewardsSymbol}`}
                />
                <Box title="Stakes">
                    <div className="flex flex-col items-center gap-4">
                        <h1>List of stakes</h1>
                        {extendedStakes?.map((extendedStake) => (
                            <StakeCard
                                key={extendedStake.stakedIotaId}
                                extendedStake={extendedStake}
                                onDetailsClick={viewStakeDetails}
                            />
                        ))}
                    </div>
                </Box>
                <Button onClick={handleNewStake}>New Stake</Button>
            </div>
            {isDialogStakeOpen && stakeDialogView && (
                <StakeDialog
                    stakedDetails={selectedStake}
                    isOpen={isDialogStakeOpen}
                    handleClose={handleCloseStakeDialog}
                    view={stakeDialogView}
                    setView={setStakeDialogView}
                    selectedValidator={selectedValidator}
                    setSelectedValidator={setSelectedValidator}
                />
            )}
        </>
    );
}

export default StakingDashboardPage;
