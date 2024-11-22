// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import {
    useFormatCoin,
    useBalance,
    CoinFormat,
    parseAmount,
    useCoinMetadata,
    ValidatorApyData,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import {
    Button,
    ButtonType,
    Input,
    InputType,
    Header,
    InfoBoxType,
    InfoBoxStyle,
    InfoBox,
} from '@iota/apps-ui-kit';
import { Field, type FieldProps, useFormikContext } from 'formik';
import { Exclamation } from '@iota/ui-icons';
import { useCurrentAccount } from '@iota/dapp-kit';
import { StakingRewardDetails, Validator, StakedInfo, Layout, LayoutBody, LayoutFooter } from './';

export interface FormValues {
    amount: string;
}

interface EnterAmountViewProps {
    selectedValidator: string;
    onBack: () => void;
    onStake: () => void;
    showActiveStatus?: boolean;
    gasBudget?: string | number | null;
    handleClose: () => void;
    validatorApy: ValidatorApyData;
}

function EnterAmountView({
    selectedValidator: selectedValidatorAddress,
    onBack,
    onStake,
    gasBudget = 0,
    handleClose,
    validatorApy,
}: EnterAmountViewProps): JSX.Element {
    const coinType = IOTA_TYPE_ARG;
    const { data: metadata } = useCoinMetadata(coinType);
    const decimals = metadata?.decimals ?? 0;

    const account = useCurrentAccount();
    const accountAddress = account?.address;

    const { values } = useFormikContext<FormValues>();
    const amount = values.amount;

    const { data: iotaBalance } = useBalance(accountAddress!);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);

    const maxTokenBalance = coinBalance - BigInt(Number(gasBudget));
    const [maxTokenFormatted, maxTokenFormattedSymbol] = useFormatCoin(
        maxTokenBalance,
        IOTA_TYPE_ARG,
        CoinFormat.FULL,
    );

    const gasBudgetBigInt = BigInt(gasBudget ?? 0);

    const hasEnoughRemaingBalance =
        maxTokenBalance > parseAmount(values.amount, decimals) + BigInt(2) * gasBudgetBigInt;
    const shouldShowInsufficientRemainingFundsWarning =
        maxTokenFormatted >= values.amount && !hasEnoughRemaingBalance;

    return (
        <Layout>
            <Header title="Enter amount" onClose={handleClose} onBack={handleClose} titleCentered />
            <LayoutBody>
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
                                            onValueChange={(values) =>
                                                setFieldValue('amount', values.value, true)
                                            }
                                            type={InputType.NumericFormat}
                                            label="Amount"
                                            value={amount}
                                            onChange={onChange}
                                            placeholder="Enter amount to stake"
                                            errorMessage={
                                                values.amount && meta.error ? meta.error : undefined
                                            }
                                            caption={`${maxTokenFormatted} ${maxTokenFormattedSymbol} Available`}
                                        />
                                    );
                                }}
                            </Field>
                            {shouldShowInsufficientRemainingFundsWarning ? (
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
                        <StakingRewardDetails gasBudget={gasBudget} {...validatorApy} />
                    </div>
                </div>
            </LayoutBody>
            <LayoutFooter>
                <div className="flex w-full justify-between gap-sm">
                    <Button fullWidth type={ButtonType.Secondary} onClick={onBack} text="Back" />
                    <Button
                        fullWidth
                        type={ButtonType.Primary}
                        onClick={onStake}
                        disabled={!amount}
                        text="Stake"
                    />
                </div>
            </LayoutFooter>
        </Layout>
    );
}

export default EnterAmountView;
