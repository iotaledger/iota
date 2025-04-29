// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    Button,
    ButtonSize,
    ButtonType,
    DisplayStats,
    Panel,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import {
    StakeDialog,
    StakeDialogView,
    UnstakeDialog,
    useUnstakeDialog,
    UnstakeDialogView,
    useStakeDialog,
    StartStaking,
} from '@/components';
import {
    ExtendedDelegatedStake,
    formatDelegatedStake,
    useGetDelegatedStake,
    useTotalDelegatedRewards,
    useTotalDelegatedStake,
    DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    DELEGATED_STAKES_QUERY_STALE_TIME,
    StakedCard,
    useFormatCoin,
    useGetAllOwnedObjects,
    TIMELOCK_IOTA_TYPE,
    Feature,
} from '@iota/core';
import { useCurrentAccount, useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { useMemo } from 'react';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { isSupplyIncreaseVestingObject, mapTimelockObjects } from '@/lib/utils';
import { useFeature } from '@growthbook/growthbook-react';
import { useRouter } from 'next/navigation';

function StakingDashboardPage(): React.JSX.Element {
    const router = useRouter();
    const account = useCurrentAccount();
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const committeeMembers = system?.committeeMembers;
    const activeValidators = system?.activeValidators;
    const iotaClient = useIotaClient();

    const { data: timelockedObjects } = useGetAllOwnedObjects(account?.address || '', {
        StructType: TIMELOCK_IOTA_TYPE,
    });
    const hasAvailableVestedStaking = mapTimelockObjects(timelockedObjects || []).some(
        isSupplyIncreaseVestingObject,
    );
    const supplyIncreaseVestingEnabled = useFeature<boolean>(Feature.SupplyIncreaseVesting).value;

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
    const {
        isOpen: isUnstakeDialogOpen,
        openUnstakeDialog,
        defaultDialogProps,
        handleClose: handleCloseUnstakeDialog,
        setView: setUnstakeDialogView,
        setTxDigest,
    } = useUnstakeDialog();

    const { data: delegatedStakeData, refetch: refetchDelegatedStakes } = useGetDelegatedStake({
        address: account?.address || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const extendedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(extendedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(extendedStakes);
    const [totalDelegatedStakeFormatted, symbol] = useFormatCoin({ balance: totalDelegatedStake });
    const [totalDelegatedRewardsFormatted] = useFormatCoin({ balance: totalDelegatedRewards });

    const delegations = useMemo(() => {
        return delegatedStakeData?.flatMap((delegation) => {
            const isInCommittee = committeeMembers?.find(
                (member) => member.stakingPoolId === delegation.stakingPool,
            );
            const isActive = activeValidators?.find(
                (validator) => validator.stakingPoolId === delegation.stakingPool,
            );
            return delegation.stakes.map((d) => ({
                ...d,
                // flag any inactive validator for the stakeIota object
                // if the stakingPoolId is not found in the committeeMembers list flag as inactive
                activeButNotInTheCommittee: !isInCommittee && isActive,
                inactiveValidator: !isActive,
                validatorAddress: delegation.validatorAddress,
            }));
        });
    }, [activeValidators, committeeMembers, delegatedStakeData]);

    const viewStakeDetails = (extendedStake: ExtendedDelegatedStake) => {
        setStakeDialogView(StakeDialogView.Details);
        setSelectedStake(extendedStake);
    };

    function handleOnStakeSuccess(digest: string): void {
        iotaClient
            .waitForTransaction({
                digest,
            })
            .then(() => refetchDelegatedStakes());
    }

    function handleUnstakeClick() {
        setStakeDialogView(undefined);
        openUnstakeDialog();
    }

    function handleUnstakeDialogBack() {
        setStakeDialogView(StakeDialogView.Details);
        handleCloseUnstakeDialog();
    }

    function handleOnUnstakeBack(view: UnstakeDialogView): (() => void) | undefined {
        if (view === UnstakeDialogView.Unstake) {
            return handleUnstakeDialogBack;
        }
    }

    function handleOnUnstakeSuccess(tx: IotaSignAndExecuteTransactionOutput): void {
        setUnstakeDialogView(UnstakeDialogView.TransactionDetails);
        iotaClient
            .waitForTransaction({
                digest: tx.digest,
            })
            .then((tx) => {
                refetchDelegatedStakes();
                setTxDigest(tx.digest);
            });
    }

    return (
        <div className="flex justify-center">
            <div className="flex w-full flex-col gap-y-md md:w-3/4">
                {(delegatedStakeData?.length ?? 0) > 0 ? (
                    <Panel>
                        <Title
                            title="Staking"
                            trailingElement={
                                <Button
                                    onClick={() => handleNewStake()}
                                    size={ButtonSize.Small}
                                    type={ButtonType.Primary}
                                    text="Stake"
                                />
                            }
                        />
                        <div className="flex h-full w-full flex-col flex-nowrap gap-md p-md--rs">
                            <div className="flex gap-xs">
                                <DisplayStats
                                    label="Your stake"
                                    value={totalDelegatedStakeFormatted}
                                    supportingLabel={symbol}
                                />
                                <DisplayStats
                                    label="Earned"
                                    value={totalDelegatedRewardsFormatted}
                                    supportingLabel={symbol}
                                />
                            </div>
                            <Title title="In progress" size={TitleSize.Small} />
                            <div className="flex max-h-[420px] w-full flex-1 flex-col items-start overflow-auto">
                                {system &&
                                    delegations
                                        ?.filter(({ inactiveValidator }) => inactiveValidator)
                                        .map((delegation) => (
                                            <div
                                                className="w-full gap-2"
                                                key={delegation.stakedIotaId}
                                            >
                                                <StakedCard
                                                    extendedStake={delegation}
                                                    inactiveValidator
                                                    currentEpoch={Number(system.epoch)}
                                                    onClick={() => viewStakeDetails(delegation)}
                                                />
                                            </div>
                                        ))}
                                {system &&
                                    delegations
                                        ?.filter(
                                            ({ activeButNotInTheCommittee }) =>
                                                activeButNotInTheCommittee,
                                        )
                                        .map((delegation) => (
                                            <div
                                                className="w-full gap-2"
                                                key={delegation.stakedIotaId}
                                            >
                                                <StakedCard
                                                    extendedStake={delegation}
                                                    currentEpoch={Number(system.epoch)}
                                                    activeButNotInTheCommittee
                                                    onClick={() => viewStakeDetails(delegation)}
                                                />
                                            </div>
                                        ))}
                                {system &&
                                    delegations
                                        ?.filter(
                                            ({ activeButNotInTheCommittee, inactiveValidator }) =>
                                                !activeButNotInTheCommittee && !inactiveValidator,
                                        )
                                        .map((delegation) => (
                                            <div
                                                className="w-full gap-2"
                                                key={delegation.stakedIotaId}
                                            >
                                                <StakedCard
                                                    extendedStake={delegation}
                                                    currentEpoch={Number(system.epoch)}
                                                    onClick={() => viewStakeDetails(delegation)}
                                                />
                                            </div>
                                        ))}
                            </div>
                        </div>
                        {isDialogStakeOpen && (
                            <StakeDialog
                                stakedDetails={selectedStake}
                                onSuccess={handleOnStakeSuccess}
                                handleClose={handleCloseStakeDialog}
                                view={stakeDialogView}
                                setView={setStakeDialogView}
                                selectedValidator={selectedValidator}
                                setSelectedValidator={setSelectedValidator}
                                onUnstakeClick={handleUnstakeClick}
                            />
                        )}

                        {isUnstakeDialogOpen && selectedStake && (
                            <UnstakeDialog
                                extendedStake={selectedStake}
                                onBack={handleOnUnstakeBack}
                                onSuccess={handleOnUnstakeSuccess}
                                {...defaultDialogProps}
                            />
                        )}
                    </Panel>
                ) : (
                    <div className="flex h-[270px] p-lg">
                        <StartStaking />
                    </div>
                )}
                {hasAvailableVestedStaking && supplyIncreaseVestingEnabled && (
                    <div className="px-lg">
                        <Panel bgColor="bg-secondary-90 dark:bg-secondary-10">
                            <div className="py-sm">
                                <Title
                                    title="Available Vested Staking"
                                    subtitle="In progress vested staking"
                                    trailingElement={
                                        <Button
                                            onClick={() => router.push('/vesting')}
                                            size={ButtonSize.Small}
                                            type={ButtonType.Outlined}
                                            text="View"
                                        />
                                    }
                                />
                            </div>
                        </Panel>
                    </div>
                )}
            </div>
        </div>
    );
}

export default StakingDashboardPage;
