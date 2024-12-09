// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../layout';
import { Validator } from '../Staking/views/Validator';
import { useNotifications, useTimelockedUnstakeTransaction } from '@/hooks';
import {
    Collapsible,
    formatAndNormalizeObjectType,
    TimeUnit,
    useFormatCoin,
    useGetActiveValidatorsInfo,
    useGetTimeBeforeEpochNumber,
    useTimeAgo,
} from '@iota/core';
import {
    useCurrentAccount,
    useIotaClientQuery,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';
import { TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { NotificationType } from '@/stores/notificationStore';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    Panel,
    LoadingIndicator,
    KeyValueInfo,
    Header,
    Dialog,
    ButtonType,
    Button,
} from '@iota/apps-ui-kit';

interface UnstakeTimelockedObjectsDialogProps {
    onClose: () => void;
    groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
}

export function UnstakeTimelockedObjectsDialog({
    groupedTimelockedObjects,
    onClose,
    onSuccess,
}: UnstakeTimelockedObjectsDialogProps) {
    const currentAddress = useCurrentAccount()?.address ?? '';
    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionPending } =
        useSignAndExecuteTransaction();
    const { addNotification } = useNotifications();
    const { data: activeValidators } = useGetActiveValidatorsInfo();
    const stakedIotaIds = groupedTimelockedObjects.stakes.map(
        (stake) => stake.timelockedStakedIotaId,
    );
    const validatorInfo = activeValidators?.find(
        ({ iotaAddress: validatorAddress }) =>
            validatorAddress === groupedTimelockedObjects.validatorAddress,
    );
    const { data: timelockedUnstakeTx, isPending: isUnstakeTxLoading } =
        useTimelockedUnstakeTransaction(stakedIotaIds, currentAddress);

    function handleTimelockedUnstake(): void {
        if (!timelockedUnstakeTx) return;

        signAndExecuteTransaction(
            {
                transaction: timelockedUnstakeTx.transaction,
            },
            {
                onSuccess: (tx) => {
                    onSuccess(tx);
                    onClose();
                    addNotification('Unstake transaction has been sent');
                },
            },
        ).catch(() => {
            addNotification('Unstake transaction was not sent', NotificationType.Error);
        });
    }

    function handleCopySuccess() {
        addNotification('Copied to clipboard');
    }

    return (
        <Dialog open onOpenChange={onClose}>
            <DialogLayout>
                <Header title="Unstake" onClose={onClose} />
                <DialogLayoutBody>
                    <div className="flex flex-col gap-md">
                        <Validator
                            address={groupedTimelockedObjects.validatorAddress}
                            isSelected
                            showActiveStatus
                        />

                        <TimelockStakeObjectsOverview
                            rewardsPool={validatorInfo?.rewardsPool}
                            groupedTimelockedObjects={groupedTimelockedObjects}
                            handleCopySuccess={handleCopySuccess}
                        />
                    </div>
                </DialogLayoutBody>
                <DialogLayoutFooter>
                    <Button
                        onClick={handleTimelockedUnstake}
                        text="Unstake"
                        icon={
                            isTransactionPending || isUnstakeTxLoading ? (
                                <LoadingIndicator />
                            ) : undefined
                        }
                        disabled={
                            !timelockedUnstakeTx || isTransactionPending || isUnstakeTxLoading
                        }
                        type={ButtonType.Secondary}
                        fullWidth
                    />
                </DialogLayoutFooter>
            </DialogLayout>
        </Dialog>
    );
}

interface TimelockStakeObjectsOverviewProps {
    groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
    handleCopySuccess: () => void;
    rewardsPool: string | undefined;
}

function TimelockStakeObjectsOverview({
    groupedTimelockedObjects: { stakes, validatorAddress, stakeRequestEpoch },
    handleCopySuccess,
    rewardsPool,
}: TimelockStakeObjectsOverviewProps) {
    const stakeId = stakes[0].timelockedStakedIotaId;
    const totalStakedAmount = stakes.reduce((acc, stake) => acc + parseInt(stake.principal), 0);
    const totalRewards = stakes.reduce(
        (acc, stake) => acc + (stake.status === 'Active' ? parseInt(stake.estimatedReward) : 0),
        0,
    );
    const totalUnstakedTimelocked = totalStakedAmount + totalRewards;
    const [totalUnstakedTimelockedFormatted] = useFormatCoin(
        totalUnstakedTimelocked,
        IOTA_TYPE_ARG,
    );
    const [rewardsPoolFormatted] = useFormatCoin(rewardsPool, IOTA_TYPE_ARG);
    const [rewardsFormatted] = useFormatCoin(totalRewards, IOTA_TYPE_ARG);
    const [stakedFormatted, stakeToken] = useFormatCoin(totalStakedAmount, IOTA_TYPE_ARG);

    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const epoch: number = parseInt(system?.epoch || '0');
    const { data: currentEpochEndTime } = useGetTimeBeforeEpochNumber(epoch + 1 || 0);
    const currentEpochEndTimeAgo = useTimeAgo({
        timeFrom: currentEpochEndTime,
        endLabel: '--',
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_HOUR,
    });

    const currentEpochEndTimeFormatted =
        currentEpochEndTime > 0 ? currentEpochEndTimeAgo : `Epoch #${epoch}`;

    return (
        <>
            {stakeId && (
                <ValidatorStakingData
                    key={stakeId}
                    validatorAddress={validatorAddress}
                    stakeId={stakeId}
                    isUnstake
                />
            )}

            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Current Epoch Ends"
                        value={currentEpochEndTimeFormatted}
                        fullwidth
                    />

                    <KeyValueInfo
                        keyText="Your Stake"
                        value={stakedFormatted}
                        supportingLabel={stakeToken}
                        fullwidth
                    />

                    <KeyValueInfo
                        keyText="Rewards Earned"
                        value={rewardsFormatted}
                        supportingLabel={stakeToken}
                        fullwidth
                    />

                    <KeyValueInfo
                        keyText="Total Unstaked Timelocked"
                        value={totalUnstakedTimelockedFormatted}
                        supportingLabel={stakeToken}
                        fullwidth
                    />
                </div>
            </Panel>

            <Panel hasBorder>
                <div className="flex flex-col gap-y-sm p-md">
                    <KeyValueInfo
                        keyText="Stake Request Epoch"
                        value={stakeRequestEpoch}
                        fullwidth
                    />
                    {rewardsPool && (
                        <KeyValueInfo
                            keyText="Rewards Pool"
                            value={rewardsPoolFormatted}
                            supportingLabel={stakeToken}
                            fullwidth
                        />
                    )}
                    <KeyValueInfo keyText="Total Stakes" value={stakes.length} fullwidth />
                </div>
            </Panel>

            {stakes.map((stake, index) => {
                const structTag = stake.label
                    ? formatAndNormalizeObjectType(stake.label).structTag
                    : '';
                return (
                    <Collapsible
                        defaultOpen
                        key={stake.timelockedStakedIotaId}
                        title={'Stake' + ' ' + (index + 1)}
                    >
                        <Panel>
                            <div className="flex flex-col gap-y-sm p-md--rs py-sm">
                                <KeyValueInfo
                                    keyText="Stake ID"
                                    value={formatAddress(stake.timelockedStakedIotaId)}
                                    valueHoverTitle={stake.timelockedStakedIotaId}
                                    onCopySuccess={handleCopySuccess}
                                    copyText={stake.timelockedStakedIotaId}
                                    fullwidth
                                />
                                <KeyValueInfo
                                    keyText="Expiration time"
                                    value={stake.expirationTimestampMs}
                                    fullwidth
                                />
                                {stake.label && structTag && (
                                    <KeyValueInfo
                                        keyText="Label"
                                        value={structTag}
                                        copyText={stake.label}
                                        valueHoverTitle={stake.label}
                                        onCopySuccess={handleCopySuccess}
                                        fullwidth
                                    />
                                )}
                                <KeyValueInfo keyText="Status" value={stake.status} fullwidth />
                            </div>
                        </Panel>
                    </Collapsible>
                );
            })}
        </>
    );
}
