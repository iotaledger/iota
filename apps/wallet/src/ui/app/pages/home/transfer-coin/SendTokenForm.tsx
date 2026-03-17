// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAccounts, useActiveAddress } from '_hooks';
import { Loading } from '_components';
import {
    useGetAllCoins,
    useCoinMetadata,
    useFormatCoin,
    AddressInput,
    SendTokenFormInput,
    sumCoinBalances,
    getGasBudgetErrorMessage,
    type SendCoinTransaction,
    NO_BALANCE_GENERIC_MESSAGE,
    type SendTokenFormValues,
    RECEIVING_ADDRESS_FIELD_IDS,
} from '@iota/core';
import { CoinFormat, IOTA_TYPE_ARG, safeParseAmount } from '@iota/iota-sdk/utils';
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
    const { data: accounts } = useAccounts();
    const activeAddress = useActiveAddress();
    const coinMetadata = useCoinMetadata(coinType);
    const { values, isValid, isSubmitting, setFieldValue } =
        useFormikContext<SendTokenFormValues>();

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
        format: CoinFormat.Full,
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

    const isSendingToIotaName = values.resolvedAddress ? values.resolvedAddress.length > 0 : false;

    const matchedAccountWithNickname = isSendingToIotaName
        ? accounts?.find(({ nickname }) => {
              if (!nickname || !values.to || values.to === '@') return;

              return denormalizeIotaName(values.to) === denormalizeIotaName(nickname);
          })
        : undefined;

    const showNicknameWarning =
        matchedAccountWithNickname && matchedAccountWithNickname.address !== values.resolvedAddress;

    function denormalizeIotaName(name: string) {
        return name.replace(/^@/, '').replace(/\.iota$/i, '');
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
            <div className="flex h-full w-full flex-col">
                <Form autoComplete="off" noValidate className="flex-1">
                    <div className="flex h-full w-full flex-col gap-md" data-amp-mask>
                        <SendTokenFormInput
                            name="amount"
                            coinType={coinType}
                            coins={coins}
                            onActionClick={onMaxTokenButtonClick}
                            isMaxActionDisabled={isMaxActionDisabled}
                            totalGas={transactionData?.gasSummary?.totalGas}
                            coinMetadata={coinMetadata.data}
                        />
                        <AddressInput
                            {...RECEIVING_ADDRESS_FIELD_IDS}
                            placeholder="Enter Address"
                        />
                        {showNicknameWarning && (
                            <InfoBox
                                type={InfoBoxType.Warning}
                                title="Sending to an IOTA Name"
                                supportingText={`You have an account named '${matchedAccountWithNickname.nickname}' but you are sending to the IOTA Name '${values.to}'. These are different addresses. Please verify the recipient’s address before sending.`}
                                style={InfoBoxStyle.Elevated}
                                icon={<Exclamation />}
                            />
                        )}
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
