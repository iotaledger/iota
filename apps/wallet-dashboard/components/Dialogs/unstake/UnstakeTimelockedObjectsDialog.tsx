// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { StakeRewardsPanel, ValidatorStakingData } from '@/components';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../layout';
import { Validator } from '../Staking/views/Validator';
import { useNotifications } from '@/hooks';
import { Collapsible, useFormatCoin, useGetActiveValidatorsInfo } from '@iota/core';
import { ExtendedDelegatedTimelockedStake, TimelockedStakedObjectsGrouped } from '@/lib/utils';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    Panel,
    LoadingIndicator,
    KeyValueInfo,
    Header,
    ButtonType,
    Button,
} from '@iota/apps-ui-kit';
import { Transaction } from '@iota/iota-sdk/transactions';

interface UnstakeTimelockedObjectsDialogProps {
    onClose: () => void;
    groupedTimelockedObjects: TimelockedStakedObjectsGrouped;
    unstakeTx: Transaction | undefined;
    handleUnstake: () => void;
    isUnstakeTxLoading: boolean;
    isTxPending: boolean;
    onBack?: () => void;
}

export function UnstakeTimelockedObjectsDialog({
    groupedTimelockedObjects,
    onClose,
    onBack,
    unstakeTx,
    handleUnstake,
    isUnstakeTxLoading,
    isTxPending,
}: UnstakeTimelockedObjectsDialogProps) {
    const { addNotification } = useNotifications();
    const { data: activeValidators } = useGetActiveValidatorsInfo();
    const validatorInfo = activeValidators?.find(
        ({ iotaAddress: validatorAddress }) =>
            validatorAddress === groupedTimelockedObjects.validatorAddress,
    );
    const stakes = groupedTimelockedObjects.stakes;
    const stakeId = stakes[0].timelockedStakedIotaId;
    const totalStakedAmount = stakes.reduce((acc, stake) => acc + parseInt(stake.principal), 0);
    const totalRewards = stakes.reduce(
        (acc, stake) => acc + (stake.status === 'Active' ? parseInt(stake.estimatedReward) : 0),
        0,
    );

    const [rewardsPoolFormatted, rewardsToken] = useFormatCoin(
        validatorInfo?.rewardsPool,
        IOTA_TYPE_ARG,
    );

    function handleCopySuccess() {
        addNotification('Copied to clipboard');
    }

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
                <Button
                    onClick={handleUnstake}
                    text="Unstake"
                    icon={isUnstakeTxLoading ? <LoadingIndicator /> : undefined}
                    disabled={!unstakeTx || isTxPending || isUnstakeTxLoading}
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
                        value={stake.expirationTimestampMs}
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
