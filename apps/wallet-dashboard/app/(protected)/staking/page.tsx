// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import {
    Button,
    ButtonSize,
    ButtonType,
    DisplayStats,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
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
} from '@iota/core';
import { useCurrentAccount, useIotaClient, useIotaClientQuery } from '@iota/dapp-kit';
import { IotaSystemStateSummary } from '@iota/iota-sdk/client';
import { Info } from '@iota/apps-ui-icons';
import { useMemo } from 'react';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';

function StakingDashboardPage(): React.JSX.Element {
    const account = useCurrentAccount();
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const activeValidators = (system as IotaSystemStateSummary)?.activeValidators;
    const iotaClient = useIotaClient();

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
            return delegation.stakes.map((d) => ({
                ...d,
                // flag any inactive validator for the stakeIota object
                // if the stakingPoolId is not found in the activeValidators list flag as inactive
                inactiveValidator: !activeValidators?.find(
                    ({ stakingPoolId }) => stakingPoolId === delegation.stakingPool,
                ),
                validatorAddress: delegation.validatorAddress,
            }));
        });
    }, [activeValidators, delegatedStakeData]);

    // Check if there are any inactive validators
    const hasInactiveValidatorDelegation = delegations?.some(
        ({ inactiveValidator }) => inactiveValidator,
    );

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
            <div className="w-full md:w-3/4">
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
                                {hasInactiveValidatorDelegation ? (
                                    <div className="mb-3">
                                        <InfoBox
                                            type={InfoBoxType.Default}
                                            title="Earn with active validators"
                                            supportingText="Unstake IOTA from the inactive validators and stake on an active validator to start earning rewards again."
                                            icon={<Info />}
                                            style={InfoBoxStyle.Elevated}
                                        />
                                    </div>
                                ) : null}
                                <div className="w-full gap-2">
                                    {system &&
                                        delegations
                                            ?.filter(({ inactiveValidator }) => inactiveValidator)
                                            .map((delegation) => (
                                                <StakedCard
                                                    extendedStake={delegation}
                                                    currentEpoch={Number(system.epoch)}
                                                    key={delegation.stakedIotaId}
                                                    inactiveValidator
                                                    onClick={() => viewStakeDetails(delegation)}
                                                />
                                            ))}
                                </div>
                                <div className="w-full gap-2">
                                    {system &&
                                        delegations
                                            ?.filter(({ inactiveValidator }) => !inactiveValidator)
                                            .map((delegation) => (
                                                <StakedCard
                                                    extendedStake={delegation}
                                                    currentEpoch={Number(system.epoch)}
                                                    key={delegation.stakedIotaId}
                                                    onClick={() => viewStakeDetails(delegation)}
                                                />
                                            ))}
                                </div>
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
            </div>
        </div>
    );
}

export default StakingDashboardPage;
