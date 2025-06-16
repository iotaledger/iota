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
    safeParseAmount,
    sumCoinBalances,
    getGasBudgetErrorMessage,
    type SendCoinTransaction,
    NO_BALANCE_GENERIC_MESSAGE,
} from '@iota/core';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { Form, useFormikContext } from 'formik';
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
import type { UseQueryResult } from '@tanstack/react-query';
import type { FormValues } from './index';
import type { CoinStruct } from '@iota/iota-sdk/client';
import { useMemo } from 'react';

export type SendTokenFormProps = {
    coinType: string;
    sendCoinTransactionQuery: UseQueryResult<SendCoinTransaction>;
    selectedCoinsQuery: UseQueryResult<CoinStruct[]>;
    onNext: () => void;
};

export function SendTokenForm({
    coinType,
    sendCoinTransactionQuery,
    selectedCoinsQuery,
    onNext,
}: SendTokenFormProps) {
    const activeAddress = useActiveAddress();
    const coinMetadata = useCoinMetadata(coinType);
    const { values, isValid, isSubmitting, setFieldValue } = useFormikContext<FormValues>();

    const { data: iotaCoins = [], isPending: iotaCoinsIsPending } = useGetAllCoins(
        IOTA_TYPE_ARG,
        activeAddress!,
    );

    const { data: coins = [], isPending: coinsIsPending } = selectedCoinsQuery;

    const coinBalance = sumCoinBalances(coins);
    const iotaBalance = sumCoinBalances(iotaCoins);

    const [tokenBalance, _, queryResult] = useFormatCoin({
        balance: coinBalance,
        coinType,
        format: CoinFormat.FULL,
    });

    const coinDecimals = coinMetadata.data?.decimals ?? 0;

    const hasAmount = values.amount.length > 0;
    const hasIotaBalance = iotaBalance > BigInt(0);
    const amount = safeParseAmount(coinType === IOTA_TYPE_ARG ? values.amount : '0', coinDecimals);
    const isPayAllIota = amount === coinBalance && coinType === IOTA_TYPE_ARG;

    const {
        data: transactionData,
        isError: isSendCoinErrored,
        error: sendCoinError,
        isLoading: isBuildingTransaction,
    } = sendCoinTransactionQuery;

    const gasBudgetEst = transactionData?.gasSummary?.totalGas;
    const gasAmount = BigInt(gasBudgetEst ?? '0');
    const canPay = amount !== null ? iotaBalance > amount + gasAmount : false;
    const hasEnoughBalance = !(hasAmount && !canPay && !isPayAllIota);

    // remove the comma from the token balance
    const formattedTokenBalance = tokenBalance.replace(/,/g, '');

    const isMaxActionDisabled = isPayAllIota || queryResult.isPending || !coinBalance;

    async function onMaxTokenButtonClick() {
        await setFieldValue('amount', formattedTokenBalance);
    }

    const gasError = useMemo(() => {
        if (!isBuildingTransaction && isSendCoinErrored) {
            return getGasBudgetErrorMessage(sendCoinError);
        }

        if (iotaBalance === BigInt(0)) {
            return NO_BALANCE_GENERIC_MESSAGE;
        }
    }, [iotaBalance, isBuildingTransaction, isSendCoinErrored, sendCoinError]);

    return (
        <Loading
            loading={
                queryResult.isPending ||
                coinMetadata.isPending ||
                iotaCoinsIsPending ||
                coinsIsPending
            }
        >
            <div className="flex h-full w-full flex-col">
                <Form autoComplete="off" noValidate className="flex-1">
                    <div className="flex h-full w-full flex-col gap-md">
                        <SendTokenFormInput
                            name="amount"
                            coinType={coinType}
                            coins={coins}
                            onActionClick={onMaxTokenButtonClick}
                            isMaxActionDisabled={isMaxActionDisabled}
                            totalGas={transactionData?.gasSummary?.totalGas}
                            coinMetadata={coinMetadata.data}
                        />
                        <AddressInput name="to" placeholder="Enter Address" />
                    </div>
                </Form>

                <div className="pt-xs">
                    {gasError && (
                        <div className="mb-sm">
                            <InfoBox
                                type={InfoBoxType.Error}
                                supportingText={gasError}
                                style={InfoBoxStyle.Elevated}
                                icon={<Exclamation />}
                            />
                        </div>
                    )}
                    <Button
                        onClick={onNext}
                        htmlType={ButtonHtmlType.Submit}
                        type={ButtonType.Primary}
                        icon={isBuildingTransaction ? <LoadingIndicator /> : undefined}
                        iconAfterText
                        disabled={
                            !hasIotaBalance ||
                            !isValid ||
                            isSubmitting ||
                            !hasEnoughBalance ||
                            gasBudgetEst === '' ||
                            gasBudgetEst === undefined ||
                            !coinMetadata ||
                            coinMetadata.data === null
                        }
                        text="Review"
                        fullWidth
                    />
                </div>
            </div>
        </Loading>
    );
}
