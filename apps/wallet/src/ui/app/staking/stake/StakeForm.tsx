// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    CoinFormat,
    createStakeTransaction,
    getGasSummary,
    parseAmount,
    useCoinMetadata,
    useFormatCoin,
} from '@iota/core';
import { Field, type FieldProps, Form, useFormikContext } from 'formik';
import { memo, useEffect, useMemo } from 'react';
import { useActiveAddress, useTransactionDryRun } from '../../hooks';
import { type FormValues } from './StakingCard';
import { InfoBox, InfoBoxStyle, InfoBoxType, Input, InputType } from '@iota/apps-ui-kit';
import { StakeTxnInfo } from '../../components/receipt-card/StakeTxnInfo';
import { Transaction } from '@iota/iota-sdk/transactions';
import { Exclamation } from '@iota/ui-icons';

export interface StakeFromProps {
    validatorAddress: string;
    coinBalance: bigint;
    coinType: string;
    epoch?: string | number;
}

function StakeForm({ validatorAddress, coinBalance, coinType, epoch }: StakeFromProps) {
    const { values, setFieldValue } = useFormikContext<FormValues>();
    const activeAddress = useActiveAddress();
    const { data: metadata } = useCoinMetadata(coinType);
    const decimals = metadata?.decimals ?? 0;

    const transaction = useMemo(() => {
        if (!values.amount || !decimals) return null;
        if (Number(values.amount) < 0) return null;
        const amountWithoutDecimals = parseAmount(values.amount, decimals);
        return createStakeTransaction(amountWithoutDecimals, validatorAddress);
    }, [values.amount, validatorAddress, decimals]);

    const { data: txDryRunResponse } = useTransactionDryRun(
        activeAddress ?? undefined,
        transaction ?? new Transaction(),
    );

    const gasSummary = txDryRunResponse ? getGasSummary(txDryRunResponse) : undefined;

    const stakeAllTransaction = useMemo(() => {
        return createStakeTransaction(coinBalance, validatorAddress);
    }, [coinBalance, validatorAddress]);

    const { data: stakeAllTransactionDryRun } = useTransactionDryRun(
        activeAddress ?? undefined,
        stakeAllTransaction,
    );

    const gasBudget = BigInt(stakeAllTransactionDryRun?.input.gasData.budget ?? 0);

    // do not remove: gasBudget field is used in the validation schema apps/wallet/src/ui/app/staking/stake/utils/validation.ts
    useEffect(() => {
        setFieldValue('gasBudget', gasBudget);
    }, [gasBudget]);

    const maxTokenBalance = coinBalance - gasBudget;
    const [maxTokenFormatted, symbol] = useFormatCoin(maxTokenBalance, coinType, CoinFormat.FULL);

    const hasEnoughRemaingBalance =
        maxTokenBalance > parseAmount(values.amount, decimals) + BigInt(2) * gasBudget;
    const shouldShowInsufficientRemainingFundsWarning =
        Number(maxTokenFormatted) >= Number(values.amount) && !hasEnoughRemaingBalance;

    return (
        <Form
            className="flex w-full flex-1 flex-col flex-nowrap items-center gap-md"
            autoComplete="off"
        >
            <Field name="amount">
                {({
                    field: { onChange, ...field },
                    form: { setFieldValue },
                    meta,
                }: FieldProps<FormValues>) => {
                    return (
                        <Input
                            {...field}
                            onValueChange={(values) => setFieldValue('amount', values.value, true)}
                            type={InputType.NumericFormat}
                            name="amount"
                            placeholder={`0 ${symbol}`}
                            value={values.amount}
                            caption={coinBalance ? `${maxTokenFormatted} ${symbol} Available` : ''}
                            suffix={' ' + symbol}
                            errorMessage={values.amount && meta.error ? meta.error : undefined}
                            label="Amount"
                        />
                    );
                }}
            </Field>
            {shouldShowInsufficientRemainingFundsWarning ? (
                <InfoBox
                    type={InfoBoxType.Error}
                    supportingText="You have selected an amount that will leave you with insufficient funds to pay for gas fees for unstaking or any other transactions."
                    style={InfoBoxStyle.Elevated}
                    icon={<Exclamation />}
                />
            ) : null}
            <StakeTxnInfo startEpoch={epoch} gasSummary={transaction ? gasSummary : undefined} />
        </Form>
    );
}

export default memo(StakeForm);
