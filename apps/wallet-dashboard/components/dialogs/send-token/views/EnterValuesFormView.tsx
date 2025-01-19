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
    Header,
} from '@iota/apps-ui-kit';
import { useIotaClientQuery } from '@iota/dapp-kit';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form, FormikProvider, useFormik, useFormikContext } from 'formik';
import { Exclamation } from '@iota/apps-ui-icons';
import { UseQueryResult } from '@tanstack/react-query';
import { FormDataValues } from '../interfaces';
import { INITIAL_VALUES } from '../constants';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';

interface EnterValuesFormProps {
    coin: CoinBalance;
    activeAddress: string;
    initialFormValues: FormDataValues;
    setFormData: React.Dispatch<React.SetStateAction<FormDataValues>>;
    setSelectedCoin: React.Dispatch<React.SetStateAction<CoinBalance>>;
    onNext: () => void;
    onClose: () => void;
}

interface FormInputsProps {
    coinType: string;
    coinDecimals: number;
    coinBalance: bigint;
    iotaBalance: bigint;
    formattedTokenBalance: string;
    symbol: string;
    activeAddress: string;
    coins: CoinStruct[];
    queryResult: UseQueryResult<CoinMetadata | null>;
    formattedAmount: bigint;
    hasEnoughBalance: boolean;
    isPayAllIota: boolean;
}

function totalBalance(coins: CoinStruct[]): bigint {
    return coins.reduce((partialSum, c) => partialSum + getBalanceFromCoinStruct(c), BigInt(0));
}
function getBalanceFromCoinStruct(coin: CoinStruct): bigint {
    return BigInt(coin.balance);
}

function FormInputs({
    coinDecimals,
    coinBalance,
    formattedTokenBalance,
    symbol,
    activeAddress,
    coins,
    queryResult,
    formattedAmount,
    hasEnoughBalance,
    isPayAllIota,
}: FormInputsProps): React.JSX.Element {
    const { setFieldValue, values } = useFormikContext<FormDataValues>();

    async function onMaxTokenButtonClick() {
        await setFieldValue('amount', formattedTokenBalance);
    }

    const isMaxActionDisabled =
        formattedAmount === coinBalance || queryResult.isPending || !coinBalance;

    return (
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
    );
}

export function EnterValuesFormView({
    coin,
    activeAddress,
    setFormData,
    setSelectedCoin,
    onNext,
    initialFormValues,
    onClose,
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

    const formik = useFormik({
        initialValues: initialFormValues,
        validationSchema: validationSchemaStepOne,
        enableReinitialize: true,
        validateOnChange: false,
        validateOnBlur: false,
        onSubmit: handleFormSubmit,
    });

    async function handleFormSubmit({ to, amount, gasBudgetEst }: FormDataValues) {
        const formattedAmount = parseAmount(amount, coinDecimals).toString();
        const data = {
            to,
            amount: formattedAmount,
            gasBudgetEst,
        };
        setFormData(data);
        onNext();
    }

    const coinType = coin.coinType;
    const formattedAmount = parseAmount(formik.values.amount, coinDecimals);
    const isPayAllIota = formattedAmount === coinBalance && coinType === IOTA_TYPE_ARG;

    const hasEnoughBalance =
        isPayAllIota ||
        iotaBalance >
            BigInt(formik.values.gasBudgetEst ?? '0') +
                (coinType === IOTA_TYPE_ARG ? formattedAmount : 0n);

    if (coinsBalanceIsPending || coinsIsPending || iotaCoinsIsPending) {
        return (
            <div className="flex h-full w-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    return (
        <FormikProvider value={formik}>
            <Header title={'Send'} onClose={onClose} />
            <DialogLayoutBody>
                <CoinSelector
                    activeCoinType={coin.coinType}
                    coins={coinsBalance ?? []}
                    onClick={(coinType) => {
                        setFormData(INITIAL_VALUES);
                        const selectedCoin = coinsBalance?.find(
                            (coinBalance) => coinBalance.coinType === coinType,
                        );
                        if (selectedCoin) {
                            setSelectedCoin(selectedCoin);
                        }
                    }}
                />

                <FormInputs
                    hasEnoughBalance={hasEnoughBalance}
                    formattedAmount={formattedAmount}
                    isPayAllIota={isPayAllIota}
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
            </DialogLayoutBody>
            <DialogLayoutFooter>
                <Button
                    onClick={formik.submitForm}
                    htmlType={ButtonHtmlType.Submit}
                    type={ButtonType.Primary}
                    disabled={
                        !formik.isValid ||
                        formik.isSubmitting ||
                        !hasEnoughBalance ||
                        formik.values.gasBudgetEst === ''
                    }
                    text="Review"
                    fullWidth
                />
            </DialogLayoutFooter>
        </FormikProvider>
    );
}
