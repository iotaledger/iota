// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_app/hooks/useActiveAddress';
import BottomMenuLayout, { Content, Menu } from '_app/shared/bottom-menu-layout';
import { Button } from '_app/shared/ButtonUI';
import { Text } from '_app/shared/text';
import { AddressInput, Alert, Loading } from '_components';
import { GAS_SYMBOL } from '_src/ui/app/redux/slices/iota-objects/Coin';
import { InputWithAction } from '_src/ui/app/shared/InputWithAction';
import {
    useGetAllCoins,
    CoinFormat,
    createTokenTransferTransaction,
    isIotaNSName,
    useCoinMetadata,
    useFormatCoin,
    useIotaNSEnabled,
    parseAmount,
} from '@iota/core';
import { useIotaClient } from '@iota/dapp-kit';
import { ArrowRight16 } from '@iota/icons';
import { type CoinStruct } from '@iota/iota.js/client';
import { IOTA_TYPE_ARG } from '@iota/iota.js/utils';
import { useQuery } from '@tanstack/react-query';
import { Field, Form, Formik, useFormikContext } from 'formik';
import { useEffect, useMemo } from 'react';

import { createValidationSchemaStepOne } from './validation';

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

function GasBudgetEstimation({
    coinDecimals,
    coins,
}: {
    coinDecimals: number;
    coins: CoinStruct[];
}) {
    const activeAddress = useActiveAddress();
    const { values, setFieldValue } = useFormikContext<FormValues>();
    const iotaNSEnabled = useIotaNSEnabled();

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

            let to = values.to;
            if (iotaNSEnabled && isIotaNSName(values.to)) {
                const address = await client.resolveNameServiceAddress({
                    name: values.to,
                });
                if (!address) {
                    throw new Error('IotaNS name not found.');
                }
                to = address;
            }

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
        setFieldValue('gasBudgetEst', formattedGas, true);
    }, [formattedGas, setFieldValue, values.amount]);

    return (
        <div className="my-2 flex w-full justify-between gap-2 px-2">
            <div className="flex gap-1">
                <Text variant="body" color="gray-80" weight="medium">
                    Estimated Gas Fees
                </Text>
            </div>
            <Text variant="body" color="gray-90" weight="medium">
                {formattedGas ? formattedGas + ' ' + GAS_SYMBOL : '--'}
            </Text>
        </div>
    );
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
    const iotaNSEnabled = useIotaNSEnabled();

    const validationSchemaStepOne = useMemo(
        () =>
            createValidationSchemaStepOne(client, iotaNSEnabled, coinBalance, symbol, coinDecimals),
        [client, coinBalance, symbol, coinDecimals, iotaNSEnabled],
    );

    // remove the comma from the token balance
    const formattedTokenBalance = tokenBalance.replace(/,/g, '');
    const initAmountBig = parseAmount(initialAmount, coinDecimals);

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
                validateOnMount
                validateOnChange
                onSubmit={async ({ to, amount, isPayAllIota, gasBudgetEst }: FormValues) => {
                    if (!coins || !iotaCoins) return;
                    const coinsIDs = [...coins]
                        .sort((a, b) => Number(b.balance) - Number(a.balance))
                        .map(({ coinObjectId }) => coinObjectId);

                    if (iotaNSEnabled && isIotaNSName(to)) {
                        const address = await client.resolveNameServiceAddress({
                            name: to,
                        });
                        if (!address) {
                            throw new Error('IotaNS name not found.');
                        }
                        to = address;
                    }

                    const data = {
                        to,
                        amount,
                        isPayAllIota,
                        coins,
                        coinIds: coinsIDs,
                        gasBudgetEst,
                    };
                    onSubmit(data);
                }}
            >
                {({ isValid, isSubmitting, setFieldValue, values, submitForm, validateField }) => {
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

                    return (
                        <BottomMenuLayout>
                            <Content>
                                <Form autoComplete="off" noValidate>
                                    <div className="flex w-full flex-grow flex-col">
                                        <div className="mb-2.5 px-2">
                                            <Text variant="caption" color="steel" weight="semibold">
                                                Select Coin Amount to Send
                                            </Text>
                                        </div>

                                        <InputWithAction
                                            data-testid="coin-amount-input"
                                            type="numberInput"
                                            name="amount"
                                            placeholder="0.00"
                                            prefix={values.isPayAllIota ? '~ ' : ''}
                                            actionText="Max"
                                            suffix={` ${symbol}`}
                                            actionType="button"
                                            allowNegative={false}
                                            decimals
                                            rounded="lg"
                                            dark
                                            onActionClicked={async () => {
                                                // using await to make sure the value is set before the validation
                                                await setFieldValue(
                                                    'amount',
                                                    formattedTokenBalance,
                                                );
                                                validateField('amount');
                                            }}
                                            actionDisabled={
                                                parseAmount(values?.amount, coinDecimals) ===
                                                    coinBalance ||
                                                queryResult.isPending ||
                                                !coinBalance
                                            }
                                        />
                                    </div>
                                    {!hasEnoughBalance && isValid ? (
                                        <div className="mt-3">
                                            <Alert>Insufficient IOTA to cover transaction</Alert>
                                        </div>
                                    ) : null}

                                    {coins ? (
                                        <GasBudgetEstimation
                                            coinDecimals={coinDecimals}
                                            coins={coins}
                                        />
                                    ) : null}

                                    <div className="mt-7.5 flex w-full flex-col gap-2.5">
                                        <div className="px-2 tracking-wider">
                                            <Text variant="caption" color="steel" weight="semibold">
                                                Enter Recipient Address
                                            </Text>
                                        </div>
                                        <div className="relative flex w-full flex-col items-center">
                                            <Field
                                                component={AddressInput}
                                                name="to"
                                                placeholder="Enter Address"
                                            />
                                        </div>
                                    </div>
                                </Form>
                            </Content>
                            <Menu
                                stuckClass="sendCoin-cta"
                                className="mx-0 w-full gap-2.5 px-0 pb-0"
                            >
                                <Button
                                    type="submit"
                                    onClick={submitForm}
                                    variant="primary"
                                    loading={isSubmitting}
                                    disabled={
                                        !isValid ||
                                        isSubmitting ||
                                        !hasEnoughBalance ||
                                        values.gasBudgetEst === ''
                                    }
                                    size="tall"
                                    text="Review"
                                    after={<ArrowRight16 />}
                                />
                            </Menu>
                        </BottomMenuLayout>
                    );
                }}
            </Formik>
        </Loading>
    );
}
