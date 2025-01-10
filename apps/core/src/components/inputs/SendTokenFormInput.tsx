// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonPill, Input, InputType } from '@iota/apps-ui-kit';
import { CoinStruct } from '@iota/iota-sdk/client';
import { useFormatCoin, useGasBudgetEstimation } from '../../hooks';
import { useEffect } from 'react';
import { useField, useFormikContext } from 'formik';
import { TokenForm } from '../../forms';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export interface SendTokenInputProps {
    coins: CoinStruct[];
    symbol: string;
    coinDecimals: number;
    activeAddress: string;
    to: string;
    onActionClick: () => Promise<void>;
    isMaxActionDisabled?: boolean;
    name: string;
    isPayAllIota: boolean;
}

export function SendTokenFormInput({
    coins,
    to,
    symbol,
    coinDecimals,
    activeAddress,
    onActionClick,
    isMaxActionDisabled,
    name,
    isPayAllIota,
}: SendTokenInputProps) {
    const { values, setFieldValue, isSubmitting, validateField } = useFormikContext<TokenForm>();
    const { data: gasBudgetEstimation } = useGasBudgetEstimation({
        coinDecimals,
        coins: coins ?? [],
        activeAddress,
        to: to,
        amount: values.amount,
        isPayAllIota,
    });
    const [formattedGasBudgetEstimation, gasToken] = useFormatCoin(
        gasBudgetEstimation,
        IOTA_TYPE_ARG,
    );

    const [field, meta, helpers] = useField<string>(name);
    const errorMessage = meta.error;
    const isActionButtonDisabled = isSubmitting || isMaxActionDisabled;

    const renderAction = () => (
        <ButtonPill disabled={isActionButtonDisabled} onClick={onActionClick}>
            Max
        </ButtonPill>
    );

    const gasAmount = formattedGasBudgetEstimation
        ? formattedGasBudgetEstimation + ' ' + gasToken
        : undefined;

    // gasBudgetEstimation should change when the amount above changes
    useEffect(() => {
        setFieldValue('gasBudgetEst', gasBudgetEstimation, false);
    }, [gasBudgetEstimation, setFieldValue, values.amount]);

    return (
        <Input
            type={InputType.NumericFormat}
            name={field.name}
            onBlur={field.onBlur}
            value={field.value}
            caption="Est. Gas Fees:"
            placeholder="0.00"
            label="Send Amount"
            suffix={` ${symbol}`}
            prefix={isPayAllIota ? '~ ' : undefined}
            allowNegative={false}
            errorMessage={errorMessage}
            amountCounter={!errorMessage ? (coins ? gasAmount : '--') : undefined}
            trailingElement={renderAction()}
            decimalScale={coinDecimals ? undefined : 0}
            thousandSeparator
            onValueChange={async (values) => {
                await helpers.setValue(values.value);
                validateField(name);
            }}
        />
    );
}
