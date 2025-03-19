// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonPill, Input, InputType } from '@iota/apps-ui-kit';
import { CoinStruct } from '@iota/iota-sdk/client';
import {
    CoinFormat,
    IOTA_COIN_METADATA,
    useCoinMetadata,
    useFormatCoin,
    useSendCoinTransaction,
} from '../../hooks';
import { useEffect } from 'react';
import { useField, useFormikContext } from 'formik';
import { TokenForm } from '../../forms';
import { parseAmount } from '../../utils';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { ERROR_ID_TO_MESSAGE, GAS_BALANCE_TOO_LOW_ID } from '../../constants';

export interface SendTokenInputProps {
    coins: CoinStruct[];
    coinType: string;
    activeAddress: string;
    onActionClick: () => Promise<void>;
    isMaxActionDisabled?: boolean;
    name: string;
    setIsBuildingTransaction: React.Dispatch<React.SetStateAction<boolean>>;
}

export function SendTokenFormInput({
    coins,
    coinType,
    activeAddress,
    onActionClick,
    isMaxActionDisabled,
    name,
    setIsBuildingTransaction,
}: SendTokenInputProps) {
    const { values, setFieldValue, isSubmitting, validateField, setErrors } =
        useFormikContext<TokenForm>();
    const {
        data: transactionData,
        isError: isSendCoinErrored,
        error: sendCoinError,
        isLoading: isBuildingTransaction,
    } = useSendCoinTransaction({
        coins,
        coinType,
        senderAddress: activeAddress,
        recipientAddress: values.to,
        amount: values.amount,
    });

    const totalGas = transactionData?.gasSummary?.totalGas;
    const { data: coinMetadata } = useCoinMetadata(coinType);
    const coinDecimals = coinMetadata?.decimals ?? 0;
    const symbol = coinMetadata?.symbol ?? IOTA_COIN_METADATA.symbol;

    const [formattedGasBudgetEstimation, gasToken] = useFormatCoin({
        balance: transactionData?.gasSummary?.totalGas,
        format: CoinFormat.FULL,
    });

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

    const totalBalance = coins.reduce((acc, { balance }) => {
        return BigInt(acc) + BigInt(balance);
    }, BigInt(0));
    const approximation =
        parseAmount(values.amount, coinDecimals) === totalBalance && coinType === IOTA_TYPE_ARG;
    // gasBudgetEstimation should change when the amount above changes
    useEffect(() => {
        setFieldValue('gasBudgetEst', totalGas, false);
    }, [totalGas, setFieldValue, values.amount]);

    useEffect(() => {
        setIsBuildingTransaction(isBuildingTransaction);

        if (
            !isBuildingTransaction &&
            isSendCoinErrored &&
            sendCoinError.message.includes(GAS_BALANCE_TOO_LOW_ID)
        ) {
            setErrors({ gasBudgetEst: ERROR_ID_TO_MESSAGE[GAS_BALANCE_TOO_LOW_ID] });
        }
    }, [sendCoinError, isSendCoinErrored, setErrors, isBuildingTransaction]);

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
