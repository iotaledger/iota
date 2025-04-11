// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinBalance } from '@iota/iota-sdk/client';
import {
    AddressInput,
    CoinFormat,
    CoinSelector,
    getGasBudgetErrorMessage,
    NO_BALANCE_GENERIC_MESSAGE,
    safeParseAmount,
    SendCoinTransaction,
    SendTokenFormInput,
    useCoinMetadata,
    useFormatCoin,
    useGetAllBalances,
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
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form, useFormikContext } from 'formik';
import { Exclamation } from '@iota/apps-ui-icons';
import { FormDataValues } from '../interfaces';
import { DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { useMemo } from 'react';
import { UseQueryResult } from '@tanstack/react-query';

interface EnterValuesFormProps {
    coin: CoinBalance;
    activeAddress: string;
    onCoinSelect: (coin: CoinBalance) => void;
    onNext: () => void;
    onClose: () => void;
    sendCoinTransactionQuery: UseQueryResult<SendCoinTransaction>;
    coinBalance: bigint;
    iotaBalance: bigint;
    showLoading: boolean;
}

export function EnterValuesFormView({
    coin,
    activeAddress,
    onCoinSelect: onCoinSelect,
    onNext,
    onClose,
    sendCoinTransactionQuery,
    coinBalance,
    iotaBalance,
    showLoading,
}: EnterValuesFormProps): JSX.Element {
    const formik = useFormikContext<FormDataValues>();

    // Get all coins of the type
    const { data: coins = [], isPending: coinsIsPending } = useGetAllCoins(
        coin.coinType,
        activeAddress,
    );
    const { data: coinsBalance, isPending: coinsBalanceIsPending } =
        useGetAllBalances(activeAddress);

    const coinType = coin.coinType;

    const [tokenBalance, _, queryResult] = useFormatCoin({
        balance: coinBalance,
        coinType,
        format: CoinFormat.FULL,
    });

    const coinMetadata = useCoinMetadata(coinType);
    const coinDecimals = coinMetadata.data?.decimals ?? 0;

    const gasBudgetError = useMemo(() => {
        const { isLoading, isError } = sendCoinTransactionQuery;
        if (!isLoading && isError) {
            const gasBudgetError = getGasBudgetErrorMessage(sendCoinTransactionQuery.error);
            if (gasBudgetError) {
                return gasBudgetError;
            }
        }

        if (iotaBalance === BigInt(0)) {
            return NO_BALANCE_GENERIC_MESSAGE;
        }
    }, [sendCoinTransactionQuery, iotaBalance]);

    const gasBudgetEst = sendCoinTransactionQuery.data?.gasSummary?.totalGas;

    const hasAmount = formik.values.amount.length > 0;
    const amount = safeParseAmount(
        coinType === IOTA_TYPE_ARG ? formik.values.amount : '0',
        coinDecimals,
    );
    const isPayAllIota = amount === coinBalance && coinType === IOTA_TYPE_ARG;
    const gasAmount = BigInt(gasBudgetEst ?? '0');

    const canPay = amount !== null ? iotaBalance > amount + gasAmount : false;
    const hasEnoughBalance = !(hasAmount && !canPay && !isPayAllIota);

    const isMaxActionDisabled = isPayAllIota || queryResult.isPending || !coinBalance;

    if (coinsBalanceIsPending || coinsIsPending || showLoading) {
        return (
            <div className="flex h-full w-full items-center justify-center">
                <LoadingIndicator />
            </div>
        );
    }

    async function onMaxTokenButtonClick() {
        const formattedTokenBalance = tokenBalance.replace(/,/g, '');
        await formik.setFieldValue('amount', formattedTokenBalance);
    }

    return (
        <>
            <Header title={'Send'} onClose={onClose} />
            <DialogLayoutBody>
                <div className="flex h-full w-full flex-col gap-md">
                    <CoinSelector
                        activeCoinType={coin.coinType}
                        coins={coinsBalance ?? []}
                        onClick={(coinType) => {
                            const selectedCoin = coinsBalance?.find(
                                (coinBalance) => coinBalance.coinType === coinType,
                            );
                            if (selectedCoin) {
                                onCoinSelect(selectedCoin);
                            }
                        }}
                    />

                    <Form autoComplete="off" noValidate className="flex-1" onSubmit={onNext}>
                        <div className="flex h-full w-full flex-col gap-md">
                            <SendTokenFormInput
                                name="amount"
                                coinType={coinType}
                                coins={coins ?? []}
                                onActionClick={onMaxTokenButtonClick}
                                isMaxActionDisabled={isMaxActionDisabled}
                                totalGas={sendCoinTransactionQuery.data?.gasSummary?.totalGas}
                            />
                            <AddressInput name="to" placeholder="Enter Address" />
                        </div>
                    </Form>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                {gasBudgetError ? (
                    <div className="mb-sm">
                        <InfoBox
                            type={InfoBoxType.Error}
                            supportingText={gasBudgetError}
                            style={InfoBoxStyle.Elevated}
                            icon={<Exclamation />}
                        />
                    </div>
                ) : null}
                <Button
                    onClick={onNext}
                    htmlType={ButtonHtmlType.Submit}
                    type={ButtonType.Primary}
                    icon={sendCoinTransactionQuery.isLoading ? <LoadingIndicator /> : undefined}
                    iconAfterText
                    disabled={
                        !formik.isValid ||
                        formik.isSubmitting ||
                        !hasEnoughBalance ||
                        !!gasBudgetError ||
                        !gasBudgetEst
                    }
                    text="Review"
                    fullWidth
                />
            </DialogLayoutFooter>
        </>
    );
}
