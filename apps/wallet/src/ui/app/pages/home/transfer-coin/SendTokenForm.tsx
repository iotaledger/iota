// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import { AddressInput, Loading } from '_components';
import { GAS_SYMBOL } from '_src/ui/app/redux/slices/iota-objects/Coin';
import {
    useGetAllCoins,
    CoinFormat,
    createTokenTransferTransaction,
    useCoinMetadata,
    useFormatCoin,
    parseAmount,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { type CoinStruct } from '@iota/iota-sdk/client';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import { Field, Form, Formik, useFormikContext } from 'formik';
import { useEffect, useMemo } from 'react';

import { createValidationSchemaStepOne } from './validation';
import {
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    InputType,
    Button,
    ButtonType,
    ButtonHtmlType,
    ButtonPill,
} from '@iota/apps-ui-kit';
import { FormInput } from './FormInput';
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

function useGasBudgetEstimation({
    coinDecimals,
    coins,
}: {
    coinDecimals: number;
    coins: CoinStruct[];
}) {
    const activeAddress = useActiveAddress();
    const { values, setFieldValue } = useFormikContext<FormValues>();

    const client = useIotaClient();
    const { data: gasBudget } = useQuery({
        // eslint-disable-next-line @tanstack/query/exhaustive-deps
        queryKey: [
            'transaction-gas-budget-estimate',
            {
                to: values.to,
                amount: values.amount,
                coins,
                activeAddress,
                coinDecimals,
            },
        ],
        queryFn: async () => {
            if (!values.amount || !values.to || !coins || !activeAddress) {
                return null;
            }

            const to = values.to;

            const tx = createTokenTransferTransaction({
                to,
                amount: values.amount,
                coinType: IOTA_TYPE_ARG,
                coinDecimals,
                isPayAllIota: values.isPayAllIota,
                coins,
            });

            tx.setSender(activeAddress);
            await tx.build({ client });
            return tx.blockData.gasConfig.budget;
        },
    });

    const [formattedGas] = useFormatCoin(gasBudget, IOTA_TYPE_ARG);
    // gasBudgetEstimation should change when the amount above changes

    useEffect(() => {
        setFieldValue('gasBudgetEst', formattedGas, false);
    }, [formattedGas, setFieldValue, values.amount]);

    return formattedGas ? formattedGas + ' ' + GAS_SYMBOL : '--';
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
        () => createValidationSchemaStepOne(coinBalance, symbol, coinDecimals),
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
                {({ isValid, isSubmitting, setFieldValue, values, submitForm }) => {
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
                        await setFieldValue('amount', formattedTokenBalance, true);
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
                                            type={InfoBoxType.Warning}
                                            supportingText="Insufficient IOTA to cover transaction"
                                            style={InfoBoxStyle.Elevated}
                                            icon={<Exclamation />}
                                        />
                                    ) : null}

                                    <SendTokenFormInput
                                        coinDecimals={coinDecimals}
                                        symbol={symbol}
                                        coins={coins}
                                        values={values}
                                        onActionClick={onMaxTokenButtonClick}
                                        isActionButtonDisabled={isMaxActionDisabled}
                                    />
                                    <Field
                                        component={AddressInput}
                                        allowNegative={false}
                                        name="to"
                                        placeholder="Enter Address"
                                        shouldValidateManually
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

interface SendTokenInputProps {
    coinDecimals: number;
    coins?: CoinStruct[];
    symbol: string;
    values: {
        amount: string;
        isPayAllIota: boolean;
    };
    onActionClick: () => Promise<void>;
    isActionButtonDisabled?: boolean | 'auto';
}

function SendTokenFormInput({
    coinDecimals,
    coins,
    values,
    symbol,
    onActionClick,
    isActionButtonDisabled,
}: SendTokenInputProps) {
    const gasBudgetEstimation = useGasBudgetEstimation({
        coinDecimals: coinDecimals,
        coins: coins ?? [],
    });

    return (
        <FormInput
            type={InputType.NumericFormat}
            name="amount"
            label="Send Amount"
            placeholder="0.00"
            caption="Est. Gas Fees:"
            suffix={` ${symbol}`}
            decimals
            allowNegative={false}
            prefix={values.isPayAllIota ? '~ ' : undefined}
            amountCounter={coins ? gasBudgetEstimation : '--'}
            renderAction={(isButtonDisabled) => (
                <ButtonPill
                    disabled={
                        isActionButtonDisabled === 'auto'
                            ? isButtonDisabled
                            : isActionButtonDisabled
                    }
                    onClick={onActionClick}
                >
                    Max
                </ButtonPill>
            )}
        />
    );
}
