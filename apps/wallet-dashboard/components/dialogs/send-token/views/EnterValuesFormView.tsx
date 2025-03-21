// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinBalance, CoinStruct } from '@iota/iota-sdk/client';
import {
    AddressInput,
    CoinFormat,
    CoinSelector,
    createValidationSchemaSendTokenForm,
    getGasBudgetErrorMessage,
    safeParseAmount,
    SendCoinTransaction,
    SendTokenFormInput,
    useCoinMetadata,
    useFormatCoin,
    useGetAllBalances,
    useGetAllCoins,
    useSendCoinTransaction,
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
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form, FormikProvider, useFormik, useFormikContext } from 'formik';
import { Exclamation } from '@iota/apps-ui-icons';
import { FormDataValues } from '../interfaces';
import { INITIAL_VALUES } from '../constants';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { useEffect } from 'react';

interface EnterValuesFormProps {
    coin: CoinBalance;
    activeAddress: string;
    initialFormValues: FormDataValues;
    setFormData: React.Dispatch<React.SetStateAction<FormDataValues>>;
    setSelectedCoin: React.Dispatch<React.SetStateAction<CoinBalance>>;
    onNext: () => void;
    onClose: () => void;
}

function totalBalance(coins: CoinStruct[]): bigint {
    return coins.reduce((partialSum, c) => partialSum + getBalanceFromCoinStruct(c), BigInt(0));
}
function getBalanceFromCoinStruct(coin: CoinStruct): bigint {
    return BigInt(coin.balance);
}

interface FormInputsProps {
    coinType: string;
    formattedTokenBalance: string;
    coins: CoinStruct[];
    isMaxActionDisabled: boolean;
    transactionData?: SendCoinTransaction;
}
function FormInputs({
    coinType,
    formattedTokenBalance,
    coins,
    isMaxActionDisabled,
    transactionData,
}: FormInputsProps): React.JSX.Element {
    const { setFieldValue } = useFormikContext<FormDataValues>();

    async function onMaxTokenButtonClick() {
        await setFieldValue('amount', formattedTokenBalance);
    }

    return (
        <Form autoComplete="off" noValidate className="flex-1">
            <div className="flex h-full w-full flex-col gap-md">
                <SendTokenFormInput
                    name="amount"
                    coinType={coinType}
                    coins={coins}
                    onActionClick={onMaxTokenButtonClick}
                    isMaxActionDisabled={isMaxActionDisabled}
                    transactionData={transactionData}
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

    const { data: coinsBalance, isPending: coinsBalanceIsPending } = useGetAllBalances();

    const iotaCoins = iotaCoinsData;
    const coins = coinsData;
    const coinBalance = totalBalance(coins || []);
    const iotaBalance = totalBalance(iotaCoins || []);
    const coinType = coin.coinType;

    const [tokenBalance, symbol, queryResult] = useFormatCoin({
        balance: coinBalance,
        coinType,
        format: CoinFormat.FULL,
    });

    const coinMetadata = useCoinMetadata(coinType);
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

    const {
        data: transactionData,
        isError: isSendCoinErrored,
        error: sendCoinError,
        isLoading: isBuildingTransaction,
    } = useSendCoinTransaction({
        coins: coins ?? [],
        coinType,
        senderAddress: activeAddress || '',
        recipientAddress: formik.values.to,
        amount: formik.values.amount,
    });

    useEffect(() => {
        if (!isBuildingTransaction && isSendCoinErrored) {
            const gasBudgetError = getGasBudgetErrorMessage(sendCoinError);
            if (gasBudgetError) {
                formik.setFieldError('gasBudgetEst', gasBudgetError);
            }
        }

        // eslint-disable-next-line react-hooks/exhaustive-deps
    }, [sendCoinError, isSendCoinErrored, isBuildingTransaction]);

    async function handleFormSubmit({ to, amount, gasBudgetEst }: FormDataValues) {
        const data = {
            to,
            amount,
            gasBudgetEst,
        };
        setFormData(data);
        onNext();
    }

    const hasAmount = formik.values.amount.length > 0;
    const amount = safeParseAmount(
        coinType === IOTA_TYPE_ARG ? formik.values.amount : '0',
        coinDecimals,
    );
    const isPayAllIota = amount === coinBalance && coinType === IOTA_TYPE_ARG;
    const gasAmount = BigInt(formik.values.gasBudgetEst ?? '0');

    const canPay = amount !== null ? iotaBalance > amount + gasAmount : false;
    const hasEnoughBalance = !(hasAmount && !canPay && !isPayAllIota);

    const isMaxActionDisabled = isPayAllIota || queryResult.isPending || !coinBalance;

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
                <div className="flex h-full w-full flex-col gap-md">
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
                        isMaxActionDisabled={isMaxActionDisabled}
                        coinType={coin.coinType}
                        formattedTokenBalance={formattedTokenBalance}
                        coins={coins ?? []}
                        transactionData={transactionData}
                    />
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                {formik.errors.gasBudgetEst ? (
                    <div className="mb-sm">
                        <InfoBox
                            type={InfoBoxType.Error}
                            supportingText={formik.errors.gasBudgetEst}
                            style={InfoBoxStyle.Elevated}
                            icon={<Exclamation />}
                        />
                    </div>
                ) : null}
                <Button
                    onClick={formik.submitForm}
                    htmlType={ButtonHtmlType.Submit}
                    type={ButtonType.Primary}
                    icon={isBuildingTransaction ? <LoadingIndicator /> : undefined}
                    iconAfterText
                    disabled={
                        !formik.isValid ||
                        formik.isSubmitting ||
                        !hasEnoughBalance ||
                        formik.values.gasBudgetEst === '' ||
                        formik.values.gasBudgetEst === undefined
                    }
                    text="Review"
                    fullWidth
                />
            </DialogLayoutFooter>
        </FormikProvider>
    );
}
