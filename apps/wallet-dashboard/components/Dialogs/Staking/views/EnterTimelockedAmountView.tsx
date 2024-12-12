// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect, useState } from 'react';
import {
    useFormatCoin,
    CoinFormat,
    useStakeTxnInfo,
    GroupedTimelockObject,
    useGetAllOwnedObjects,
    TIMELOCK_IOTA_TYPE,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    Button,
    ButtonType,
    KeyValueInfo,
    Panel,
    Divider,
    Input,
    InputType,
    Header,
    InfoBoxType,
    InfoBoxStyle,
    InfoBox,
} from '@iota/apps-ui-kit';
import { Field, type FieldProps, useFormikContext } from 'formik';
import { Exclamation, Loader } from '@iota/ui-icons';
import {
    useCurrentAccount,
    useIotaClientQuery,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';

import { Validator } from './Validator';
import { StakedInfo } from './StakedInfo';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import {
    useGetCurrentEpochStartTimestamp,
    useNewStakeTimelockedTransaction,
    useNotifications,
} from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';
import { prepareObjectsForTimelockedStakingTransaction } from '@/lib/utils';

export interface FormValues {
    amount: string;
}

interface EnterTimelockedAmountViewProps {
    selectedValidator: string;
    maxStakableTimelockedAmount: bigint;
    amountWithoutDecimals: bigint;
    senderAddress: string;
    onBack: () => void;
    handleClose: () => void;
    onSuccess: (digest: string) => void;
}

function EnterTimelockedAmountView({
    selectedValidator,
    maxStakableTimelockedAmount,
    amountWithoutDecimals,
    senderAddress,
    onBack,
    handleClose,
    onSuccess,
}: EnterTimelockedAmountViewProps): JSX.Element {
    const account = useCurrentAccount();
    const accountAddress = account?.address;
    const { addNotification } = useNotifications();
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();
    const [groupedTimelockObjects, setGroupedTimelockObjects] = useState<GroupedTimelockObject[]>(
        [],
    );
    const { data: newStakeData, isLoading: isTransactionLoading } =
        useNewStakeTimelockedTransaction(selectedValidator, senderAddress, groupedTimelockObjects);
    const { data: currentEpochMs } = useGetCurrentEpochStartTimestamp();
    const { data: timelockedObjects } = useGetAllOwnedObjects(senderAddress, {
        StructType: TIMELOCK_IOTA_TYPE,
    });

    useEffect(() => {
        if (timelockedObjects && currentEpochMs) {
            const groupedTimelockObjects = prepareObjectsForTimelockedStakingTransaction(
                timelockedObjects,
                amountWithoutDecimals,
                currentEpochMs,
            );
            setGroupedTimelockObjects(groupedTimelockObjects);
        }
    }, [timelockedObjects, currentEpochMs, amountWithoutDecimals]);

    const { values, errors, resetForm } = useFormikContext<FormValues>();
    const amount = values.amount;
    const hasGroupedTimelockObjects = groupedTimelockObjects.length > 0;

    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const [gas, symbol] = useFormatCoin(newStakeData?.gasBudget ?? 0, IOTA_TYPE_ARG);

    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin(
        maxStakableTimelockedAmount,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    const caption = `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`;

    const { stakedRewardsStartEpoch, timeBeforeStakeRewardsRedeemableAgoDisplay } = useStakeTxnInfo(
        system?.epoch,
    );

    function handleStake(): void {
        if (groupedTimelockObjects.length === 0) {
            addNotification('Invalid stake amount. Please try again.', NotificationType.Error);
            return;
        }
        if (!newStakeData?.transaction) {
            addNotification('Stake transaction was not created', NotificationType.Error);
            return;
        }
        signAndExecuteTransaction(
            {
                transaction: newStakeData?.transaction,
            },
            {
                onSuccess: (tx) => {
                    onSuccess?.(tx.digest);
                    addNotification('Stake transaction has been sent');
                    resetForm();
                },
                onError: () => {
                    addNotification('Stake transaction was not sent', NotificationType.Error);
                },
            },
        );
    }

    return (
        <DialogLayout>
            <Header title="Enter amount" onClose={handleClose} onBack={onBack} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col justify-between">
                    <div>
                        <div className="mb-md">
                            <Validator address={selectedValidator} isSelected showAction={false} />
                        </div>
                        <StakedInfo
                            validatorAddress={selectedValidator}
                            accountAddress={accountAddress!}
                        />
                        <div className="my-md w-full">
                            <Field name="amount">
                                {({
                                    field: { onChange, ...field },
                                    form: { setFieldValue },
                                    meta,
                                }: FieldProps<FormValues>) => {
                                    return (
                                        <Input
                                            {...field}
                                            onValueChange={({ value }) => {
                                                setFieldValue('amount', value, true);
                                            }}
                                            type={InputType.NumericFormat}
                                            label="Amount"
                                            value={amount}
                                            suffix={` ${symbol}`}
                                            placeholder="Enter amount to stake"
                                            errorMessage={
                                                values.amount && meta.error ? meta.error : undefined
                                            }
                                            caption={caption}
                                        />
                                    );
                                }}
                            </Field>
                            {!hasGroupedTimelockObjects && !isTransactionLoading ? (
                                <div className="mt-md">
                                    <InfoBox
                                        type={InfoBoxType.Error}
                                        supportingText="It is not possible to combine timelocked objects to stake the entered amount. Please try a different amount."
                                        style={InfoBoxStyle.Elevated}
                                        icon={<Exclamation />}
                                    />
                                </div>
                            ) : null}
                        </div>

                        <Panel hasBorder>
                            <div className="flex flex-col gap-y-sm p-md">
                                <KeyValueInfo
                                    keyText="Staking Rewards Start"
                                    value={stakedRewardsStartEpoch}
                                    fullwidth
                                />
                                <KeyValueInfo
                                    keyText="Redeem Rewards"
                                    value={timeBeforeStakeRewardsRedeemableAgoDisplay}
                                    fullwidth
                                />
                                <Divider />
                                <KeyValueInfo
                                    keyText="Gas fee"
                                    value={gas || '--'}
                                    supportingLabel={symbol}
                                    fullwidth
                                />
                            </div>
                        </Panel>
                    </div>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <div className="flex w-full justify-between gap-sm">
                    <Button fullWidth type={ButtonType.Secondary} onClick={onBack} text="Back" />
                    <Button
                        fullWidth
                        type={ButtonType.Primary}
                        disabled={
                            !amount ||
                            !!errors?.amount ||
                            isTransactionLoading ||
                            !hasGroupedTimelockObjects
                        }
                        onClick={handleStake}
                        text="Stake"
                        icon={
                            isTransactionLoading ? (
                                <Loader className="animate-spin" data-testid="loading-indicator" />
                            ) : null
                        }
                        iconAfterText
                    />
                </div>
            </DialogLayoutFooter>
        </DialogLayout>
    );
}

export default EnterTimelockedAmountView;
