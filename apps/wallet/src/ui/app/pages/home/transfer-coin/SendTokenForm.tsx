// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { Loading } from '_components';
import {
    useGetAllCoins,
    CoinFormat,
    useCoinMetadata,
    useFormatCoin,
    parseAmount,
    AddressInput,
    SendTokenFormInput,
    useGasBudgetEstimation,
    createValidationSchemaSendTokenForm,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Field, FieldInputProps, Form, Formik } from 'formik';
import { useMemo } from 'react';

import {
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Button,
    ButtonType,
    ButtonHtmlType,
} from '@iota/apps-ui-kit';
import { Exclamation } from '@iota/ui-icons';

const INITIAL_VALUES = {
    to: '',
    amount: '',
    isPayAllIota: false,
    gasBudgetEst: '',
};

export type FormValues = typeof INITIAL_VALUES;

export type SubmitProps = {
    to: string;
    amount: string;
    isPayAllIota: boolean;
    coinIds: string[];
    coins: CoinStruct[];
    gasBudgetEst: string;
};

export type SendTokenFormProps = {
    coinType: string;
    onSubmit: (values: SubmitProps) => void;
    initialAmount: string;
    initialTo: string;
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
export function SendTokenForm({
    coinType,
    onSubmit,
    initialAmount = '',
    initialTo = '',
}: SendTokenFormProps) {
    const client = useIotaClient();
    const activeAddress = useActiveAddress();
    // Get all coins of the type
    const { data: coinsData, isPending: coinsIsPending } = useGetAllCoins(coinType, activeAddress!);

    const { data: iotaCoinsData, isPending: iotaCoinsIsPending } = useGetAllCoins(
        IOTA_TYPE_ARG,
        activeAddress!,
    );

    const iotaCoins = iotaCoinsData;
    const coins = coinsData;
    const coinBalance = totalBalance(coins || []);
    const iotaBalance = totalBalance(iotaCoins || []);

    const coinMetadata = useCoinMetadata(coinType);
    const coinDecimals = coinMetadata.data?.decimals ?? 0;

    const [tokenBalance, symbol, queryResult] = useFormatCoin(
        coinBalance,
        coinType,
        CoinFormat.FULL,
    );

    const validationSchemaStepOne = useMemo(
        () => createValidationSchemaSendTokenForm(coinBalance, symbol, coinDecimals),
        [client, coinBalance, symbol, coinDecimals],
    );

    // remove the comma from the token balance
    const formattedTokenBalance = tokenBalance.replace(/,/g, '');
    const initAmountBig = parseAmount(initialAmount, coinDecimals);

    async function handleFormSubmit({ to, amount, isPayAllIota, gasBudgetEst }: FormValues) {
        if (!coins || !iotaCoins) return;
        const coinsIDs = [...coins]
            .sort((a, b) => Number(b.balance) - Number(a.balance))
            .map(({ coinObjectId }) => coinObjectId);

        const data = {
            to,
            amount,
            isPayAllIota,
            coins,
            coinIds: coinsIDs,
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
                initialValues={{
                    amount: initialAmount,
                    to: initialTo,
                    isPayAllIota:
                        !!initAmountBig &&
                        initAmountBig === coinBalance &&
                        coinType === IOTA_TYPE_ARG,
                    gasBudgetEst: '',
                }}
                validationSchema={validationSchemaStepOne}
                enableReinitialize
                validateOnChange={false}
                validateOnBlur={false}
                onSubmit={handleFormSubmit}
            >
                {({
                    isValid,
                    isSubmitting,
                    setFieldValue,
                    values,
                    submitForm,
                    handleBlur,
                    touched,
                    errors,
                }) => {
                    const newPayIotaAll =
                        parseAmount(values.amount, coinDecimals) === coinBalance &&
                        coinType === IOTA_TYPE_ARG;
                    if (values.isPayAllIota !== newPayIotaAll) {
                        setFieldValue('isPayAllIota', newPayIotaAll);
                    }

                    const hasEnoughBalance =
                        values.isPayAllIota ||
                        iotaBalance >
                            parseAmount(values.gasBudgetEst, coinDecimals) +
                                parseAmount(
                                    coinType === IOTA_TYPE_ARG ? values.amount : '0',
                                    coinDecimals,
                                );

                    async function onMaxTokenButtonClick() {
                        await setFieldValue('amount', formattedTokenBalance);
                    }

                    const isMaxActionDisabled =
                        parseAmount(values?.amount, coinDecimals) === coinBalance ||
                        queryResult.isPending ||
                        !coinBalance;

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

                                    <Field name="amount">
                                        {({ field }: { field: FieldInputProps<string> }) => {
                                            // TODO: needs to be updated in values
                                            const gasBudgetEstimation = useGasBudgetEstimation({
                                                coinDecimals: coinDecimals,
                                                coins: coins ?? [],
                                                activeAddress: activeAddress ?? '',
                                                to: values.to,
                                                amount: values.amount,
                                                isPayAllIota: values.isPayAllIota,
                                                setFieldValue
                                            });
                                            return (
                                            <SendTokenFormInput
                                                gasBudgetEstimation={gasBudgetEstimation}
                                                symbol={symbol}
                                                coins={coins}
                                                values={values}
                                                onActionClick={onMaxTokenButtonClick}
                                                isActionButtonDisabled={isMaxActionDisabled}
                                                value={field.value}
                                                onChange={(value) => setFieldValue('amount', value)}
                                                onBlur={handleBlur}
                                                errorMessage={
                                                    touched.amount && errors.amount
                                                        ? errors.amount
                                                        : undefined
                                                }
                                            />
                                        )}}
                                    </Field>
                                    <Field
                                        component={AddressInput}
                                        allowNegative={false}
                                        name="to"
                                        placeholder="Enter Address"
                                    />
                                </div>
                            </Form>

                            <div className="pt-xs">
                                <Button
                                    onClick={submitForm}
                                    htmlType={ButtonHtmlType.Submit}
                                    type={ButtonType.Primary}
                                    disabled={
                                        !isValid ||
                                        isSubmitting ||
                                        !hasEnoughBalance ||
                                        values.gasBudgetEst === ''
                                    }
                                    text="Review"
                                    fullWidth
                                />
                            </div>
                        </div>
                    );
                }}
            </Formik>
        </Loading>
    );
}
