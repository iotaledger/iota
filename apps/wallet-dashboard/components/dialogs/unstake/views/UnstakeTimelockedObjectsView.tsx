// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StakeRewardsPanel, ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { Validator } from '@iota/core';
import { useNewUnstakeTimelockedTransaction } from '@/hooks';
import {
    Collapsible,
    TimeUnit,
    useFormatCoin,
    useGetActiveValidatorsInfo,
    useTimeAgo,
} from '@iota/core';
import {
    ExtendedDelegatedTimelockedStake,
    TimelockedStakedObjectsGrouped,
    isSizeExceededError,
} from '@/lib/utils';
import { formatAddress } from '@iota/iota-sdk/utils';
import {
    Panel,
    LoadingIndicator,
    KeyValueInfo,
    Header,
    ButtonType,
    Button,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import toast from 'react-hot-toast';
import { ampli } from '@/lib/utils/analytics';
import { Warning } from '@iota/apps-ui-icons';
import { useEffect, useRef, useState } from 'react';

interface UnstakeTimelockedObjectsViewProps {
    onClose: () => void;
    groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
    onSuccess: (tx: IotaSignAndExecuteTransactionOutput) => void;
    onBack?: () => void;
}

const REDUCTION_STEP_SIZE = 5;

export function UnstakeTimelockedObjectsView({
    groupedTimelockedObjects,
    onClose,
    onBack,
    onSuccess,
}: UnstakeTimelockedObjectsViewProps) {
    const reductionSize = useRef(0);
    const [isMaxTransactionSizeError, setIsMaxTransactionSizeError] = useState(false);
    const activeAddress = useCurrentAccount()?.address ?? '';
    const { data: activeValidators } = useGetActiveValidatorsInfo();

    const stakes = (() => {
        if (isMaxTransactionSizeError) {
            return groupedTimelockedObjects.stakes.slice(0, -reductionSize.current);
        }
        return groupedTimelockedObjects.stakes;
    })();

    const timelockedStakedIotaIds = stakes.map((stake) => stake.timelockedStakedIotaId);

    const {
        data: unstakeData,
        isPending: isUnstakeTxPending,
        isError: isUnstakeError,
        error: unstakeError,
    } = useNewUnstakeTimelockedTransaction(activeAddress, timelockedStakedIotaIds);

    const { mutateAsync: signAndExecuteTransaction, isPending: isTransactionPending } =
        useSignAndExecuteTransaction();

    const validatorInfo = activeValidators?.find(
        ({ iotaAddress: validatorAddress }) =>
            validatorAddress === groupedTimelockedObjects.validatorAddress,
    );

    const stakeId = stakes[0]?.timelockedStakedIotaId;
    const totalStakedAmount = stakes.reduce((acc, stake) => acc + BigInt(stake.principal), 0n);

    const totalRewards = stakes.reduce(
        (acc, stake) => acc + (stake.status === 'Active' ? parseInt(stake.estimatedReward) : 0),
        0,
    );

    const [totalStakedAmountFormatted, totalStakedAmountSymbol] = useFormatCoin({
        balance: totalStakedAmount,
    });

    const [rewardsPoolFormatted, rewardsToken] = useFormatCoin({
        balance: validatorInfo?.rewardsPool,
    });

    function handleCopySuccess() {
        toast.success('Copied to clipboard');
    }

    async function handleUnstake(): Promise<void> {
        if (!unstakeData) return;

        await signAndExecuteTransaction(
            {
                transaction: unstakeData.transaction,
            },
            {
                onSuccess: (tx) => {
                    toast.success('Unstake transaction has been sent');
                    onSuccess(tx);
                    ampli.timelockUnstake({
                        validatorAddress: groupedTimelockedObjects.validatorAddress,
                    });
                },
            },
        ).catch(() => {
            toast.error('Unstake transaction was not sent');
        });
    }

    useEffect(() => {
        if (isUnstakeError && isSizeExceededError(unstakeError)) {
            setIsMaxTransactionSizeError(true);
            reductionSize.current += REDUCTION_STEP_SIZE;
        }
        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [isUnstakeError, unstakeError]);

    return (
        <DialogLayout>
            <Header title="Unstake" onClose={onClose} onBack={onBack} />
            <DialogLayoutBody>
                <div className="flex flex-col gap-md">
                    <Validator
                        address={groupedTimelockedObjects.validatorAddress}
                        isSelected
                        showActiveStatus
                    />

                    {stakeId && (
                        <ValidatorStakingData
                            key={stakeId}
                            validatorAddress={groupedTimelockedObjects.validatorAddress}
                            stakeId={stakeId}
                            isUnstake
                        />
                    )}

                    <StakeRewardsPanel
                        stakingRewards={totalRewards}
                        totalStaked={totalStakedAmount}
                        isTimelocked
                    />

                    <Panel hasBorder>
                        <div className="flex flex-col gap-y-sm p-md">
                            <KeyValueInfo
                                keyText="Stake Request Epoch"
                                value={groupedTimelockedObjects.stakeRequestEpoch}
                                fullwidth
                            />
                            {rewardsPoolFormatted && (
                                <KeyValueInfo
                                    keyText="Rewards Pool"
                                    value={rewardsPoolFormatted}
                                    supportingLabel={rewardsToken}
                                    fullwidth
                                />
                            )}
                            <KeyValueInfo keyText="Total Stakes" value={stakes.length} fullwidth />
                        </div>
                    </Panel>

                    {stakes.map((stake, index) => (
                        <TimelockedStakeCollapsible
                            title={`Stake Nº${index + 1}`}
                            key={stake.timelockedStakedIotaId}
                            stake={stake}
                            handleCopySuccess={handleCopySuccess}
                        />
                    ))}
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                {isMaxTransactionSizeError ? (
                    <div className="mb-2">
                        <InfoBox
                            title="Partial unstake"
                            supportingText={`Due to the large number of objects, a partial unstake of ${totalStakedAmountFormatted} ${totalStakedAmountSymbol} will be attempted. After the operation is complete, you can unstake the remaining value.`}
                            style={InfoBoxStyle.Elevated}
                            type={InfoBoxType.Error}
                            icon={<Warning />}
                        />
                    </div>
                ) : null}
                <Button
                    onClick={handleUnstake}
                    text="Unstake"
                    icon={!unstakeData || isUnstakeTxPending ? <LoadingIndicator /> : undefined}
                    disabled={!unstakeData || isTransactionPending || isUnstakeTxPending}
                    type={ButtonType.Secondary}
                    fullWidth
                />
            </DialogLayoutFooter>
        </DialogLayout>
    );
}

interface TimelockedStakeCollapsibleProps {
    stake: ExtendedDelegatedTimelockedStake;
    title: string;
    handleCopySuccess: () => void;
}
function TimelockedStakeCollapsible({
    stake,
    title,
    handleCopySuccess,
}: TimelockedStakeCollapsibleProps) {
    const currentEpochEndTimeAgo = useTimeAgo({
        timeFrom: Number(stake.expirationTimestampMs),
        endLabel: '--',
        shortedTimeLabel: false,
        shouldEnd: true,
        maxTimeUnit: TimeUnit.ONE_DAY,
    });
    return (
        <Collapsible defaultOpen key={stake.timelockedStakedIotaId} title={title}>
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
                        value={currentEpochEndTimeAgo}
                        fullwidth
                    />
                    {stake.label && (
                        <KeyValueInfo
                            keyText="Label"
                            value={formatAddress(stake.label)}
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
}
