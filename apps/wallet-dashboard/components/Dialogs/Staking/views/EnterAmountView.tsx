// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import {
    useFormatCoin,
    useBalance,
    CoinFormat,
    parseAmount,
    useCoinMetadata,
    useStakeTxnInfo,
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
import { Exclamation } from '@iota/ui-icons';
import {
    useCurrentAccount,
    useIotaClientQuery,
    useSignAndExecuteTransaction,
} from '@iota/dapp-kit';

import { Validator } from './Validator';
import { StakedInfo } from './StakedInfo';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { useNewStakeTransaction, useNotifications } from '@/hooks';
import { NotificationType } from '@/stores/notificationStore';

export interface FormValues {
    amount: string;
}

interface EnterAmountViewProps {
    selectedValidator: string;
    onBack: () => void;
    showActiveStatus?: boolean;
    handleClose: () => void;
    amountWithoutDecimals: bigint;
    senderAddress: string;
    onSuccess?: (digest: string) => void;
}

function EnterAmountView({
    selectedValidator: selectedValidatorAddress,
    onBack,
    handleClose,
    amountWithoutDecimals,
    senderAddress,
    onSuccess,
}: EnterAmountViewProps): JSX.Element {
    const coinType = IOTA_TYPE_ARG;
    const { data: metadata } = useCoinMetadata(coinType);
    const decimals = metadata?.decimals ?? 0;

    const { addNotification } = useNotifications();
    const account = useCurrentAccount();
    const accountAddress = account?.address;

    const { values, errors, resetForm } = useFormikContext<FormValues>();
    const amount = values.amount;

    const { data: newStakeData, isLoading: isTransactionLoading } = useNewStakeTransaction(
        selectedValidatorAddress,
        amountWithoutDecimals,
        senderAddress,
    );

    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const { data: iotaBalance } = useBalance(accountAddress!);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);
    const { mutateAsync: signAndExecuteTransaction } = useSignAndExecuteTransaction();

    const gasBudgetBigInt = BigInt(newStakeData?.gasBudget ?? 0);
    const [gas, symbol] = useFormatCoin(newStakeData?.gasBudget, IOTA_TYPE_ARG);

    const maxTokenBalance = coinBalance - gasBudgetBigInt;
    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin(
        maxTokenBalance,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    const caption = isTransactionLoading
        ? '--'
        : `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`;

    const { stakedRewardsStartEpoch, timeBeforeStakeRewardsRedeemableAgoDisplay } = useStakeTxnInfo(
        system?.epoch,
    );

    const hasEnoughRemaingBalance =
        maxTokenBalance > parseAmount(values.amount, decimals) + BigInt(2) * gasBudgetBigInt;

    function handleStake(): void {
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
                            <Validator
                                address={selectedValidatorAddress}
                                isSelected
                                showAction={false}
                            />
                        </div>
                        <StakedInfo
                            validatorAddress={selectedValidatorAddress}
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
                                            caption={coinBalance ? caption : ''}
                                        />
                                    );
                                }}
                            </Field>
                            {!hasEnoughRemaingBalance ? (
                                <div className="mt-md">
                                    <InfoBox
                                        type={InfoBoxType.Error}
                                        supportingText="You have selected an amount that will leave you with insufficient funds to pay for gas fees for unstaking or any other transactions."
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
                        onClick={handleStake}
                        disabled={!amount || !!errors?.amount}
                        text="Stake"
                    />
                </div>
            </DialogLayoutFooter>
        </DialogLayout>
    );
}

export default EnterAmountView;
