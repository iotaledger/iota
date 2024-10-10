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
import { memo, useCallback, useMemo } from 'react';
import { useActiveAddress, useTransactionDryRun } from '../../hooks';
import { type FormValues } from './StakingCard';
import { ButtonPill, Input, InputType } from '@iota/apps-ui-kit';
import { StakeTxnInfo } from '../../components/receipt-card/StakeTxnInfo';
import { Transaction } from '@iota/iota-sdk/transactions';

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

    const gasBudget = useMemo(() => {
        return BigInt(stakeAllTransactionDryRun?.input.gasData.budget ?? 0);
    }, [stakeAllTransactionDryRun]);

    const [maxToken, symbol, queryResult] = useFormatCoin(
        coinBalance - gasBudget,
        coinType,
        CoinFormat.FULL,
    );
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
                    const setMaxToken = useCallback(() => {
                        if (!maxToken) return;
                        setFieldValue('amount', maxToken);
                    }, [maxToken, setFieldValue]);

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
                            trailingElement={
                                <ButtonPill
                                    onClick={setMaxToken}
                                    disabled={queryResult.isPending || isMaxValueSelected}
                                >
                                    Max
                                </ButtonPill>
                            }
                            errorMessage={values.amount && meta.error ? meta.error : undefined}
                            label="Amount"
                        />
                    );
                }}
            </Field>
            <StakeTxnInfo startEpoch={epoch} gasSummary={transaction ? gasSummary : undefined} />
        </Form>
    );
}

export default memo(StakeForm);
