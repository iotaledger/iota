// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonPill, Input, InputType } from '@iota/apps-ui-kit';
import { CoinStruct } from '@iota/iota-sdk/client';
import { CoinFormat, IOTA_COIN_METADATA, useCoinMetadata, useFormatCoin } from '../../hooks';
import { useField, useFormikContext } from 'formik';
import { TokenForm } from '../../forms';
import { parseAmount } from '../../utils';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

export interface SendTokenInputProps {
    coins: CoinStruct[];
    coinType: string;
    onActionClick: () => Promise<void>;
    isMaxActionDisabled?: boolean;
    name: string;
    totalGas?: string;
}

export function SendTokenFormInput({
    coins,
    coinType,
    onActionClick,
    isMaxActionDisabled,
    name,
    totalGas,
}: SendTokenInputProps) {
    const { values, isSubmitting, validateField } = useFormikContext<TokenForm>();

    const { data: coinMetadata } = useCoinMetadata(coinType);
    const coinDecimals = coinMetadata?.decimals ?? 0;
    const symbol = coinMetadata?.symbol ?? IOTA_COIN_METADATA.symbol;

    const [formattedGasBudgetEstimation, gasToken] = useFormatCoin({
        balance: totalGas,
        format: CoinFormat.FULL,
    });

    const [field, meta, helpers] = useField<string>(name);
    const errorMessage = meta.error;
    const isActionButtonDisabled = isSubmitting || isMaxActionDisabled;

    const gasAmount = formattedGasBudgetEstimation
        ? formattedGasBudgetEstimation + ' ' + gasToken
        : undefined;

    const totalBalance = coins.reduce((acc, { balance }) => {
        return BigInt(acc) + BigInt(balance);
    }, BigInt(0));

    const approximation =
        parseAmount(values.amount, coinDecimals) === totalBalance && coinType === IOTA_TYPE_ARG;

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
            prefix={approximation ? '~ ' : undefined}
            allowNegative={false}
            errorMessage={errorMessage}
            amountCounter={!errorMessage ? (coins ? gasAmount : '--') : undefined}
            trailingElement={
                <ButtonPill disabled={isActionButtonDisabled} onClick={onActionClick}>
                    Max
                </ButtonPill>
            }
            decimalScale={coinDecimals ? undefined : 0}
            thousandSeparator
            onValueChange={async (values) => {
                await helpers.setValue(values.value);
                validateField(name);
            }}
        />
    );
}
