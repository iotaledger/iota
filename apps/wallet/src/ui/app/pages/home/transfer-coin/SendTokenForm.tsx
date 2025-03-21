// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_hooks';
import { Loading } from '_components';
import {
    useGetAllCoins,
    CoinFormat,
    useCoinMetadata,
    useFormatCoin,
    AddressInput,
    SendTokenFormInput,
    createValidationSchemaSendTokenForm,
    safeParseAmount,
    useSendCoinTransaction,
    TokenForm,
    GAS_BALANCE_TOO_LOW_ID,
    ERROR_ID_TO_MESSAGE,
} from '@iota/core';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form, Formik, useFormikContext } from 'formik';
import { useEffect, useMemo } from 'react';

import {
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Button,
    ButtonType,
    ButtonHtmlType,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { Exclamation } from '@iota/apps-ui-icons';

export const INITIAL_VALUES = {
    to: '',
    amount: '',
    gasBudgetEst: '',
    coins: [],
};

export type FormValues = typeof INITIAL_VALUES;

export type SubmitProps = {
    to: string;
    amount: string;
    coins: CoinStruct[];
    gasBudgetEst: string;
};

export type SendTokenFormProps = {
    coinType: string;
    onSubmit: (values: SubmitProps) => void;
};

function totalBalance(coins: CoinStruct[]): bigint {
    return coins.reduce((partialSum, c) => partialSum + getBalanceFromCoinStruct(c), BigInt(0));
}
function getBalanceFromCoinStruct(coin: CoinStruct): bigint {
    return BigInt(coin.balance);
}

// Set the initial gasEstimation from initial amount
// base on the input amount field update the gasEstimation value
// Separating the gasEstimation from the formik context to access the input amount value and update the gasEstimation value
export function SendTokenForm({ coinType, onSubmit }: SendTokenFormProps) {
    const activeAddress = useActiveAddress();
    const { values, setErrors } = useFormikContext<TokenForm>();
    // Get all coins of the type
    const { data: coins, isPending: coinsIsPending } = useGetAllCoins(coinType, activeAddress!);

    const { data: iotaCoins, isPending: iotaCoinsIsPending } = useGetAllCoins(
        IOTA_TYPE_ARG,
        activeAddress!,
    );

    const coinBalance = totalBalance(coins || []);
    const iotaBalance = totalBalance(iotaCoins || []);

    const coinMetadata = useCoinMetadata(coinType);
    const coinDecimals = coinMetadata.data?.decimals ?? 0;

    const [tokenBalance, symbol, queryResult] = useFormatCoin({
        balance: coinBalance,
        coinType,
        format: CoinFormat.FULL,
    });

    const validationSchemaStepOne = useMemo(
        () => createValidationSchemaSendTokenForm(coinBalance, symbol, coinDecimals),
        [coinBalance, symbol, coinDecimals],
    );

    const {
        data: transactionData,
        isError: isSendCoinErrored,
        error: sendCoinError,
        isLoading: isBuildingTransaction,
    } = useSendCoinTransaction({
        coins: coins ?? [],
        coinType,
        senderAddress: activeAddress || '',
        recipientAddress: values.to,
        amount: values.amount,
    });

    useEffect(() => {
        if (
            !isBuildingTransaction &&
            isSendCoinErrored &&
            sendCoinError.message.includes(GAS_BALANCE_TOO_LOW_ID)
        ) {
            setErrors({ gasBudgetEst: ERROR_ID_TO_MESSAGE[GAS_BALANCE_TOO_LOW_ID] });
        }
    }, [sendCoinError, isSendCoinErrored, setErrors, isBuildingTransaction]);

    // remove the comma from the token balance
    const formattedTokenBalance = tokenBalance.replace(/,/g, '');

    async function handleFormSubmit({ to, amount, gasBudgetEst }: FormValues) {
        if (!coins) return;

        const data = {
            to,
            amount,
            coins,
            gasBudgetEst,
        };
        onSubmit(data);
    }

    return (
        <Loading
            loading={
                queryResult.isPending ||
                coinMetadata.isPending ||
                iotaCoinsIsPending ||
                coinsIsPending
            }
        >
            <Formik
                initialValues={INITIAL_VALUES}
                validationSchema={validationSchemaStepOne}
                enableReinitialize
                validateOnChange={false}
                validateOnBlur={false}
                onSubmit={handleFormSubmit}
            >
                {({ isValid, isSubmitting, setFieldValue, values, submitForm, errors }) => {
                    const hasAmount = values.amount.length > 0;
                    const amount = safeParseAmount(
                        coinType === IOTA_TYPE_ARG ? values.amount : '0',
                        coinDecimals,
                    );
                    const isPayAllIota = amount === coinBalance && coinType === IOTA_TYPE_ARG;
                    const gasAmount = BigInt(values.gasBudgetEst ?? '0');

                    const canPay = amount !== null ? iotaBalance > amount + gasAmount : false;
                    const hasEnoughBalance = !(hasAmount && !canPay && !isPayAllIota);

                    const isMaxActionDisabled =
                        isPayAllIota || queryResult.isPending || !coinBalance;

                    async function onMaxTokenButtonClick() {
                        await setFieldValue('amount', formattedTokenBalance);
                    }

                    return (
                        <div className="flex h-full w-full flex-col">
                            <Form autoComplete="off" noValidate className="flex-1">
                                <div className="flex h-full w-full flex-col gap-md">
                                    {!hasEnoughBalance ? (
                                        <InfoBox
                                            type={InfoBoxType.Error}
                                            supportingText="Insufficient IOTA to cover transaction"
                                            style={InfoBoxStyle.Elevated}
                                            icon={<Exclamation />}
                                        />
                                    ) : null}

                                    <SendTokenFormInput
                                        name="amount"
                                        coinType={coinType}
                                        coins={coins ?? []}
                                        onActionClick={onMaxTokenButtonClick}
                                        isMaxActionDisabled={isMaxActionDisabled}
                                        transactionData={transactionData}
                                    />
                                    <AddressInput name="to" placeholder="Enter Address" />
                                </div>
                            </Form>

                            {errors.gasBudgetEst ? (
                                <div className="mb-sm">
                                    <InfoBox
                                        type={InfoBoxType.Error}
                                        supportingText={errors.gasBudgetEst}
                                        style={InfoBoxStyle.Elevated}
                                        icon={<Exclamation />}
                                    />
                                </div>
                            ) : null}
                            <Button
                                onClick={submitForm}
                                htmlType={ButtonHtmlType.Submit}
                                type={ButtonType.Primary}
                                icon={isBuildingTransaction ? <LoadingIndicator /> : undefined}
                                iconAfterText
                                disabled={
                                    !isValid ||
                                    isSubmitting ||
                                    !hasEnoughBalance ||
                                    values.gasBudgetEst === '' ||
                                    !values.gasBudgetEst
                                }
                                text="Review"
                                fullWidth
                            />
                        </div>
                    );
                }}
            </Formik>
        </Loading>
    );
}
