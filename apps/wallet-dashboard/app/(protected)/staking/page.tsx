// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { StartStaking } from '@/components/staking-overview/StartStaking';
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
import { StakeDialog, StakeDialogView, UnstakeDialog } from '@/components';
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
import { Info } from '@iota/ui-icons';
import { useMemo } from 'react';
import { useStakeDialog } from '@/components/Dialogs/Staking/hooks/useStakeDialog';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useUnstakeDialog } from '@/components/Dialogs/unstake/hooks';
import { UnstakeDialogView } from '@/components/Dialogs/unstake/enums';

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
    const { isOpen: isUnstakeDialogOpen, setIsOpen: setIsUnstakeDialogOpen } = useUnstakeDialog();

    const { data: delegatedStakeData, refetch: refetchDelegatedStakes } = useGetDelegatedStake({
        address: account?.address || '',
        staleTime: DELEGATED_STAKES_QUERY_STALE_TIME,
        refetchInterval: DELEGATED_STAKES_QUERY_REFETCH_INTERVAL,
    });

    const extendedStakes = delegatedStakeData ? formatDelegatedStake(delegatedStakeData) : [];
    const totalDelegatedStake = useTotalDelegatedStake(extendedStakes);
    const totalDelegatedRewards = useTotalDelegatedRewards(extendedStakes);
    const [totalDelegatedStakeFormatted, symbol] = useFormatCoin(
        totalDelegatedStake,
        IOTA_TYPE_ARG,
    );
    const [totalDelegatedRewardsFormatted] = useFormatCoin(totalDelegatedRewards, IOTA_TYPE_ARG);

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
        setIsUnstakeDialogOpen(true);
    }

    function handleUnstakeDialogBack() {
        setStakeDialogView(StakeDialogView.Details);
        setIsUnstakeDialogOpen(false);
    }

    function handleOnUnstakeBack(view: UnstakeDialogView): (() => void) | undefined {
        if (view === UnstakeDialogView.Unstake) {
            return handleUnstakeDialogBack;
        }
    }

    return (
        <div className="flex justify-center">
            <div className="w-3/4">
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
                                handleClose={() => setIsUnstakeDialogOpen(false)}
                                extendedStake={selectedStake}
                                onBack={handleOnUnstakeBack}
                                view={UnstakeDialogView.Unstake}
                                onSuccess={() => {
                                    refetchDelegatedStakes();
                                    setIsUnstakeDialogOpen(false);
                                }}
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
