// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import type { IotaSignAndExecuteTransactionOutput } from '@iota/wallet-standard';
import { ValidatorStakingData } from '@/components';
import { Validator } from '@/components/Dialogs/Staking/views/Validator';
import { useState } from 'react';
import { useNotifications, useTimelockedUnstakeTransaction } from '@/hooks';
import {
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
import { Layout, LayoutBody, LayoutFooter } from '@/components/Dialogs/Staking/views/Layout';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    TitleSize,
    Title,
    Panel,
    LoadingIndicator,
    KeyValueInfo,
    Header,
    Divider,
    Dialog,
    ButtonType,
    Button,
    AccordionHeader,
    AccordionContent,
    Accordion,
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
            <Layout>
                <Header title="Unstake" onClose={onClose} />
                <LayoutBody>
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
                </LayoutBody>
                <LayoutFooter>
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
                </LayoutFooter>
            </Layout>
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
                        initialClose
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

interface CollapsibleProps {
    children: React.ReactNode;
    title?: string;
    footer?: React.ReactNode;
    initialClose?: boolean;
    titleSize?: TitleSize;
    hideArrow?: boolean;
    hideBorder?: boolean;
    render?: ({ isOpen }: { isOpen: boolean }) => React.ReactNode;
    supportingTitleElement?: React.ReactNode;
}

function Collapsible({
    title,
    footer,
    children,
    initialClose,
    titleSize = TitleSize.Medium,
    hideArrow,
    hideBorder,
    render,
    supportingTitleElement,
}: CollapsibleProps) {
    const [open, setOpen] = useState(!initialClose);
    return (
        <div className="relative w-full">
            <Accordion hideBorder={hideBorder}>
                <AccordionHeader
                    hideBorder={hideBorder}
                    hideArrow={hideArrow}
                    isExpanded={open}
                    onToggle={() => setOpen(!open)}
                >
                    {render ? (
                        render({ isOpen: open })
                    ) : (
                        <Title
                            size={titleSize}
                            title={title ?? ''}
                            supportingElement={supportingTitleElement}
                        />
                    )}
                </AccordionHeader>
                <AccordionContent isExpanded={open}>{children}</AccordionContent>
                {footer && (
                    <>
                        <Divider />
                        <div className={clsx('rounded-b-2xl')}>{footer}</div>
                    </>
                )}
            </Accordion>
        </div>
    );
}
