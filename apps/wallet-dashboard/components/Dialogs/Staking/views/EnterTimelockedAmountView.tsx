// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { useFormatCoin, CoinFormat, useStakeTxnInfo, Validator } from '@iota/core';
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
import { useCurrentAccount, useIotaClientQuery } from '@iota/dapp-kit';

import { StakedInfo } from './StakedInfo';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';

export interface FormValues {
    amount: string;
}

interface EnterTimelockedAmountViewProps {
    selectedValidator: string;
    maxStakableTimelockedAmount: bigint;
    onBack: () => void;
    onStake: () => void;
    gasBudget?: string | number | null;
    handleClose: () => void;
    hasGroupedTimelockObjects?: boolean;
    isTransactionLoading?: boolean;
}

function EnterTimelockedAmountView({
    selectedValidator: selectedValidatorAddress,
    maxStakableTimelockedAmount,
    hasGroupedTimelockObjects,
    onBack,
    onStake,
    gasBudget,
    handleClose,
    isTransactionLoading,
}: EnterTimelockedAmountViewProps): JSX.Element {
    const account = useCurrentAccount();
    const accountAddress = account?.address;

    const { values, errors } = useFormikContext<FormValues>();
    const amount = values.amount;

    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const [gas, symbol] = useFormatCoin(gasBudget ?? 0, IOTA_TYPE_ARG);

    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin(
        maxStakableTimelockedAmount,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    const caption = `${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`;

    const { stakedRewardsStartEpoch, timeBeforeStakeRewardsRedeemableAgoDisplay } = useStakeTxnInfo(
        system?.epoch,
    );

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
                        onClick={onStake}
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
