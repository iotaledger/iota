// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    CoinFormat,
    createValidationSchema,
    MIN_NUMBER_IOTA_TO_STAKE,
    parseAmount,
    StakeTransactionInfo,
    useBalance,
    useCoinMetadata,
    useFormatCoin,
    useNewStakeTransaction,
    Validator,
} from '@iota/core';
import * as Sentry from '@sentry/react';
import { ampli } from '_src/shared/analytics/ampli';
import {
    Field,
    type FieldProps,
    Form,
    type FormikHelpers,
    FormikProvider,
    useFormik,
} from 'formik';
import { memo, useEffect, useMemo } from 'react';
import { useActiveAccount, useSigner } from '_hooks';
import {
    Button,
    ButtonType,
    CardType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { Exclamation, Loader } from '@iota/apps-ui-icons';
import { ExplorerLinkHelper } from '../../components';
import { useMutation } from '@tanstack/react-query';
import { getSignerOperationErrorMessage } from '../../helpers';
import toast from 'react-hot-toast';
import { IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';
import { ValidatorFormDetail } from './ValidatorFormDetail';
import { type IotaTransactionBlockResponse } from '@iota/iota-sdk/client';

export interface StakeFromProps {
    validatorAddress: string;
    epoch?: string | number;
    onSuccess: (response: IotaTransactionBlockResponse) => void;
}

const INITIAL_VALUES = {
    amount: '',
};

type FormValues = typeof INITIAL_VALUES;

export function StakeFormComponent({ validatorAddress, epoch, onSuccess }: StakeFromProps) {
    const activeAccount = useActiveAccount();
    const activeAddress = activeAccount?.address ?? '';
    const signer = useSigner(activeAccount);
    const { data: iotaBalance, isPending: loadingIotaBalances } = useBalance(activeAddress);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);

    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const decimals = metadata?.decimals ?? 0;
    const coinSymbol = metadata?.symbol ?? '';

    // set minimum stake amount to 1 IOTA
    const minimumStake = parseAmount(MIN_NUMBER_IOTA_TO_STAKE.toString(), decimals);
    const validationSchema = useMemo(
        () => createValidationSchema(coinBalance, coinSymbol, decimals, minimumStake),
        [coinBalance, coinSymbol, decimals, minimumStake],
    );

    const { mutateAsync: stakeTokenMutateAsync, isPending: isStakeTokenTransactionPending } =
        useMutation({
            mutationFn: async (formikHelpers: FormikHelpers<FormValues>) => {
                if (!transaction || !signer) {
                    throw new Error('Failed, missing required field');
                }

                return Sentry.startSpan(
                    {
                        name: 'stake',
                    },
                    async (span) => {
                        try {
                            const tx = await signer.signAndExecuteTransaction({
                                transactionBlock: transaction,
                                options: {
                                    showInput: true,
                                    showEffects: true,
                                    showEvents: true,
                                },
                            });
                            formikHelpers.resetForm();
                            await signer.client.waitForTransaction({
                                digest: tx.digest,
                            });
                            return tx;
                        } finally {
                            span?.end();
                        }
                    },
                );
            },
            onSuccess: (_) => {
                ampli.stakedIota({
                    stakedAmount: Number(amountWithoutDecimals),
                    validatorAddress: validatorAddress || '',
                });
            },
            onError: (error) => {
                throw error;
            },
        });

    const handleSubmit = async (_: FormValues, formikHelpers: FormikHelpers<FormValues>) => {
        try {
            const response = await stakeTokenMutateAsync(formikHelpers);
            onSuccess(response);
        } catch (error) {
            toast.error(
                <div className="flex max-w-xs flex-col overflow-hidden">
                    <strong>Stake failed</strong>
                    <small className="overflow-hidden text-ellipsis">
                        {getSignerOperationErrorMessage(error)}
                    </small>
                </div>,
            );
        }
    };

    const formik = useFormik<FormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: handleSubmit,
        validateOnChange: true,
    });
    const { values, isValid, isSubmitting, setFieldValue, submitForm } = formik;
    const { amount } = values;
    const amountWithoutDecimals = parseAmount(amount, decimals);

    const {
        data: newStakeData,
        isLoading: isStakeTokenTransactionLoading,
        isError,
    } = useNewStakeTransaction(validatorAddress, amountWithoutDecimals, activeAddress);
    const transaction = newStakeData?.transaction;
    const gasSummary = newStakeData?.gasSummary;

    const { data: maxAmountTransactionData } = useNewStakeTransaction(
        validatorAddress,
        coinBalance,
        activeAddress,
    );
    const maxAmountTxGasBudget = BigInt(maxAmountTransactionData?.gasSummary?.budget ?? 0n);
    // do not remove: gasBudget field is used in the validation schema apps/core/src/utils/stake/createValidationSchema.ts
    useEffect(() => {
        setFieldValue('gasBudget', maxAmountTxGasBudget);
    }, [maxAmountTxGasBudget]);

    const maxTokenBalance = coinBalance - maxAmountTxGasBudget;
    const [maxTokenFormatted, symbol] = useFormatCoin({
        balance: maxTokenBalance,
        format: CoinFormat.FULL,
    });

    const hasEnoughRemainingBalance =
        maxTokenBalance > parseAmount(amount, decimals) + BigInt(2) * maxAmountTxGasBudget;

    const isLoading =
        loadingIotaBalances ||
        isSubmitting ||
        isStakeTokenTransactionLoading ||
        isStakeTokenTransactionPending;
    return (
        <FormikProvider value={formik}>
            <Form
                className="flex w-full flex-1 flex-col flex-nowrap items-center gap-md overflow-auto pb-sm"
                autoComplete="off"
            >
                <Validator address={validatorAddress} type={CardType.Filled} />
                <ValidatorFormDetail validatorAddress={validatorAddress} />
                <Field name="amount">
                    {({
                        field: { onChange, ...field },
                        form: { setFieldValue },
                        meta,
                    }: FieldProps<FormValues>) => {
                        return (
                            <Input
                                {...field}
                                onValueChange={(values) =>
                                    setFieldValue('amount', values.value, true)
                                }
                                type={InputType.NumericFormat}
                                name="amount"
                                placeholder={`0 ${symbol}`}
                                value={amount}
                                caption={
                                    maxAmountTxGasBudget
                                        ? `${maxTokenFormatted} ${symbol} Available`
                                        : '--'
                                }
                                suffix={' ' + symbol}
                                errorMessage={amount && meta.error ? meta.error : undefined}
                                label="Amount"
                            />
                        );
                    }}
                </Field>
                {!hasEnoughRemainingBalance ? (
                    <InfoBox
                        type={InfoBoxType.Error}
                        supportingText="You have selected an amount that will leave you with insufficient funds to pay for gas fees for unstaking or any other transactions."
                        style={InfoBoxStyle.Elevated}
                        icon={<Exclamation />}
                    />
                ) : null}
                <StakeTransactionInfo
                    startEpoch={epoch}
                    activeAddress={activeAddress}
                    gasSummary={transaction ? gasSummary : undefined}
                    renderExplorerLink={ExplorerLinkHelper}
                />
            </Form>
            <Button
                type={ButtonType.Primary}
                fullWidth
                onClick={submitForm}
                disabled={isError || !isValid || isLoading}
                text="Stake"
                icon={
                    isLoading ? (
                        <Loader className="animate-spin" data-testid="loading-indicator" />
                    ) : null
                }
                iconAfterText
            />
        </FormikProvider>
    );
}

export const StakeForm = memo(StakeFormComponent);
