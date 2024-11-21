// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinBalance, CoinMetadata, CoinStruct } from '@iota/iota-sdk/client';
import {
    AddressInput,
    CoinFormat,
    COINS_QUERY_REFETCH_INTERVAL,
    COINS_QUERY_STALE_TIME,
    CoinSelector,
    createValidationSchemaSendTokenForm,
    filterAndSortTokenBalances,
    parseAmount,
    SendTokenFormInput,
    useCoinMetadata,
    useFormatCoin,
    useGetAllCoins,
} from '@iota/core';
import {
    ButtonHtmlType,
    ButtonType,
    InfoBox,
    InfoBoxType,
    Button,
    InfoBoxStyle,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form, Formik, FormikProps } from 'formik';
import { Exclamation } from '@iota/ui-icons';
import { UseQueryResult } from '@tanstack/react-query';
import { FormDataValues } from '../interfaces';
import { INITIAL_VALUES } from '../constants';

interface EnterValuesFormProps {
    coin: CoinBalance;
    activeAddress: string;
    setFormData: React.Dispatch<React.SetStateAction<FormDataValues>>;
    setSelectedCoin: React.Dispatch<React.SetStateAction<CoinBalance>>;
    onNext: () => void;
}

interface FormInputsProps extends FormikProps<FormDataValues> {
    coinType: string;
    coinDecimals: number;
    coinBalance: bigint;
    iotaBalance: bigint;
    formattedTokenBalance: string;
    symbol: string;
    activeAddress: string;
    coins: CoinStruct[];
    queryResult: UseQueryResult<CoinMetadata | null>;
}

function totalBalance(coins: CoinStruct[]): bigint {
    return coins.reduce((partialSum, c) => partialSum + getBalanceFromCoinStruct(c), BigInt(0));
}
function getBalanceFromCoinStruct(coin: CoinStruct): bigint {
    return BigInt(coin.balance);
}

function FormInputs({
    isValid,
    isSubmitting,
    setFieldValue,
    values,
    submitForm,
    coinType,
    coinDecimals,
    coinBalance,
    iotaBalance,
    formattedTokenBalance,
    symbol,
    activeAddress,
    coins,
    queryResult,
}: FormInputsProps): React.JSX.Element {
    const isPayAllIota =
        parseAmount(values.amount, coinDecimals) === coinBalance && coinType === IOTA_TYPE_ARG;

    const hasEnoughBalance =
        isPayAllIota ||
        iotaBalance >
            parseAmount(values.gasBudgetEst, coinDecimals) +
                parseAmount(coinType === IOTA_TYPE_ARG ? values.amount : '0', coinDecimals);

    async function onMaxTokenButtonClick() {
        await setFieldValue('amount', formattedTokenBalance);
    }

    const isMaxActionDisabled =
        parseAmount(values.amount, coinDecimals) === coinBalance ||
        queryResult.isPending ||
        !coinBalance;

    return (
        <div className="flex h-full w-full flex-col">
            <Form autoComplete="off" noValidate className="flex-1">
                <div className="flex h-full w-full flex-col gap-md">
                    {!hasEnoughBalance && (
                        <InfoBox
                            type={InfoBoxType.Error}
                            supportingText="Insufficient IOTA to cover transaction"
                            style={InfoBoxStyle.Elevated}
                            icon={<Exclamation />}
                        />
                    )}

                    <SendTokenFormInput
                        name="amount"
                        to={values.to}
                        symbol={symbol}
                        coins={coins}
                        coinDecimals={coinDecimals}
                        activeAddress={activeAddress}
                        onActionClick={onMaxTokenButtonClick}
                        isMaxActionDisabled={isMaxActionDisabled}
                        isPayAllIota={isPayAllIota}
                    />
                    <AddressInput name="to" placeholder="Enter Address" />
                </div>
            </Form>

            <div className="pt-xs">
                <Button
                    onClick={submitForm}
                    htmlType={ButtonHtmlType.Submit}
                    type={ButtonType.Primary}
                    disabled={
                        !isValid || isSubmitting || !hasEnoughBalance || values.gasBudgetEst === ''
                    }
                    text="Review"
                    fullWidth
                />
            </div>
        </div>
    );
}

export function EnterValuesFormView({
    coin,
    activeAddress,
    setFormData,
    setSelectedCoin,
    onNext,
}: EnterValuesFormProps): JSX.Element {
    // Get all coins of the type
    const { data: coinsData, isPending: coinsIsPending } = useGetAllCoins(
        coin.coinType,
        activeAddress,
    );
    const { data: iotaCoinsData, isPending: iotaCoinsIsPending } = useGetAllCoins(
        IOTA_TYPE_ARG,
        activeAddress,
    );

    const { data: coinsBalance, isPending: coinsBalanceIsPending } = useIotaClientQuery(
        'getAllBalances',
        { owner: activeAddress },
        {
            enabled: !!activeAddress,
            refetchInterval: COINS_QUERY_REFETCH_INTERVAL,
            staleTime: COINS_QUERY_STALE_TIME,
            select: filterAndSortTokenBalances,
        },
    );

    const iotaCoins = iotaCoinsData;
    const coins = coinsData;
    const coinBalance = totalBalance(coins || []);
    const iotaBalance = totalBalance(iotaCoins || []);

    const [tokenBalance, symbol, queryResult] = useFormatCoin(
        coinBalance,
        coin.coinType,
        CoinFormat.FULL,
    );

    const coinMetadata = useCoinMetadata(coin.coinType);
    const coinDecimals = coinMetadata.data?.decimals ?? 0;

    const validationSchemaStepOne = createValidationSchemaSendTokenForm(
        coinBalance,
        symbol,
        coinDecimals,
    );

    const formattedTokenBalance = tokenBalance.replace(/,/g, '');

    if (coinsBalanceIsPending || coinsIsPending || iotaCoinsIsPending) {
        return (
            <div className="flex h-full w-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    async function handleFormSubmit({ to, amount, gasBudgetEst }: FormDataValues) {
        if (!coins || !iotaCoins) return;
        const coinsIDs = [...coins]
            .sort((a, b) => Number(b.balance) - Number(a.balance))
            .map(({ coinObjectId }) => coinObjectId);

        const formattedAmount = parseAmount(amount, coinDecimals).toString();

        const data = {
            to,
            amount: formattedAmount,
            coins,
            coinIds: coinsIDs,
            gasBudgetEst,
        };
        setFormData(data);
        onNext();
    }

    return (
        <div className="flex h-full w-full flex-col gap-md">
            <CoinSelector
                activeCoinType={coin.coinType}
                coins={coinsBalance ?? []}
                onClick={(coinType) => {
                    setFormData(INITIAL_VALUES);
                    const coin = coinsBalance?.find((coin) => coin.coinType === coinType);
                    setSelectedCoin(coin!);
                }}
            />

            <Formik
                initialValues={{
                    amount: '',
                    to: '',
                    gasBudgetEst: '',
                }}
                validationSchema={validationSchemaStepOne}
                enableReinitialize
                validateOnChange={false}
                validateOnBlur={false}
                onSubmit={handleFormSubmit}
            >
                {(props: FormikProps<FormDataValues>) => (
                    <FormInputs
                        {...props}
                        coinType={coin.coinType}
                        coinDecimals={coinDecimals}
                        coinBalance={coinBalance}
                        iotaBalance={iotaBalance}
                        formattedTokenBalance={formattedTokenBalance}
                        symbol={symbol}
                        activeAddress={activeAddress}
                        coins={coins ?? []}
                        queryResult={queryResult}
                    />
                )}
            </Formik>
        </div>
    );
}
