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
import { memo, useMemo } from 'react';
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
    const { values } = useFormikContext<FormValues>();
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

    const gasSummary = useMemo(() => {
        if (!txDryRunResponse) return null;
        return getGasSummary(txDryRunResponse);
    }, [txDryRunResponse]);

    const stakeAllTransaction = useMemo(() => {
        return createStakeTransaction(coinBalance, validatorAddress);
    }, [values.amount, validatorAddress, decimals]);

    const { data: stakeAllTransactionDryRun } = useTransactionDryRun(
        activeAddress ?? undefined,
        stakeAllTransaction ?? new Transaction(),
    );

    const gasBudget = BigInt(stakeAllTransactionDryRun?.input.gasData.budget ?? 0);

    const [maxToken, symbol] = useFormatCoin(coinBalance - gasBudget, coinType, CoinFormat.FULL);
    const isMaxValueSelected = values.amount === maxToken;

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
                            caption={coinBalance ? `~ ${maxToken} ${symbol} Available` : ''}
                            suffix={' ' + symbol}
                            prefix={isMaxValueSelected ? '~ ' : undefined}
                            errorMessage={values.amount && meta.error ? meta.error : undefined}
                            label="Amount"
                        />
                    );
                }}
            </Field>
            {isMaxValueSelected ? (
                <InfoBox
                    type={InfoBoxType.Error}
                    supportingText="You have selected the maximum amount. This will leave you with insufficient funds to pay for gas fees for unstaking or any other transactions."
                    style={InfoBoxStyle.Elevated}
                    icon={<Exclamation />}
                />
            ) : null}
            <StakeTxnInfo startEpoch={epoch} gasSummary={transaction ? gasSummary : undefined} />
        </Form>
    );
}

export default memo(StakeForm);
