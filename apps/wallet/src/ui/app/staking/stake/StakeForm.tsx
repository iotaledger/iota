// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createValidationSchema,
    MIN_NUMBER_IOTA_TO_STAKE,
    StakeTransactionInfo,
    useBalance,
    useCoinMetadata,
    useFormatCoin,
    useNewStakeTransaction,
    Validator,
    toast,
    useIsValidatorCommitteeMember,
    NO_BALANCE_GENERIC_MESSAGE,
    getGasBudgetErrorMessage,
    useGetValidatorsApy,
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
import { memo, useMemo } from 'react';
import { useActiveAccount, useSigner } from '_hooks';
import { useIotaClientQuery } from '@iota/dapp-kit';
import {
    Button,
    ButtonPill,
    ButtonType,
    CardType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { Exclamation, Loader, Warning } from '@iota/apps-ui-icons';
import { ExplorerLinkHelper } from '../../components';
import { useMutation } from '@tanstack/react-query';
import { getSignerOperationErrorMessage } from '../../helpers';
import { CoinFormat, IOTA_TYPE_ARG, parseAmount } from '@iota/iota-sdk/utils';
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
    const { data: iotaBalance, isPending: isIotaBalanceLoading } = useBalance(activeAddress);
    const coinBalance = BigInt(iotaBalance?.totalBalance || 0);
    const { isCommitteeMember } = useIsValidatorCommitteeMember();
    const { data: metadata } = useCoinMetadata(IOTA_TYPE_ARG);
    const decimals = metadata?.decimals ?? 0;
    const coinSymbol = metadata?.symbol ?? '';

    const { data: rollingAverageApys } = useGetValidatorsApy();
    const validatorApy = rollingAverageApys?.[validatorAddress]?.apy ?? 0;
    const { data: systemState } = useIotaClientQuery('getLatestIotaSystemState');
    const validatorName =
        systemState?.activeValidators.find((v) => v.iotaAddress === validatorAddress)?.name ?? '';

    const minimumStake = parseAmount(MIN_NUMBER_IOTA_TO_STAKE.toString(), decimals);

    const { data: minAmountTransactionData } = useNewStakeTransaction(
        validatorAddress,
        minimumStake,
        activeAddress,
    );

    const minAmountTxGasBudget = BigInt(minAmountTransactionData?.gasSummary?.budget ?? 0n);
    const availableBalance = coinBalance - minAmountTxGasBudget;
    const [availableBalanceFormatted, symbol] = useFormatCoin({
        balance: availableBalance,
        format: CoinFormat.Full,
    });

    const validationSchema = useMemo(
        () => createValidationSchema(availableBalance, coinSymbol, decimals, minimumStake),
        [availableBalance, coinSymbol, decimals, minimumStake],
    );

    const formik = useFormik<FormValues>({
        initialValues: INITIAL_VALUES,
        validationSchema: validationSchema,
        onSubmit: handleSubmit,
        validateOnMount: true,
    });
    const { values, isValid, isSubmitting, setFieldValue, submitForm } = formik;
    const { amount } = values;
    const amountWithoutDecimals = parseAmount(amount, decimals);
    const [stakedAmountFormattedPlain] = useFormatCoin({
        balance: amountWithoutDecimals,
        format: CoinFormat.Full,
        useGroupSeparator: false,
    });

    const { mutateAsync: stakeTokenMutateAsync, isPending: isStakeTokenTransactionPending } =
        useMutation({
            mutationFn: async () => {
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
                            await signer.client.waitForTransaction({
                                digest: tx.digest,
                            });
                            return { tx };
                        } finally {
                            span?.end();
                        }
                    },
                );
            },
            onError: (error) => {
                throw error;
            },
        });

    async function handleSubmit(_: FormValues, formikHelpers: FormikHelpers<FormValues>) {
        try {
            await stakeTokenMutateAsync(undefined, {
                onSuccess(data) {
                    ampli.stakedIota({
                        stakedAmount: Number(stakedAmountFormattedPlain),
                        validatorAddress: validatorAddress || '',
                        validatorAPY: validatorApy,
                        validatorName,
                    });
                    formikHelpers.resetForm();
                    onSuccess(data.tx);
                },
            });
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
    }

    const {
        data: newStakeData,
        isLoading: isStakeTokenTransactionLoading,
        isError,
        error: stakeTransactionError,
    } = useNewStakeTransaction(validatorAddress, amountWithoutDecimals, activeAddress);
    const transaction = newStakeData?.transaction;
    const gasSummary = newStakeData?.gasSummary;

    const isLoading =
        isIotaBalanceLoading ||
        isSubmitting ||
        isStakeTokenTransactionLoading ||
        isStakeTokenTransactionPending;

    const gasUnstakeBuffer = minAmountTxGasBudget * BigInt(2);
    const maxSafeAmount = availableBalance - gasUnstakeBuffer;
    const [maxSafeAmountFormatted, maxSafeAmountSymbol] = useFormatCoin({
        balance: maxSafeAmount,
        format: CoinFormat.Full,
    });
    const isUnsafeAmount =
        amountWithoutDecimals &&
        amountWithoutDecimals > maxSafeAmount &&
        amountWithoutDecimals <= availableBalance;

    const errorMessage = useMemo(() => {
        if (isError) {
            return getGasBudgetErrorMessage(stakeTransactionError) ?? NO_BALANCE_GENERIC_MESSAGE;
        } else {
            return undefined;
        }
    }, [stakeTransactionError, isError]);

    function setMaxAmount() {
        setFieldValue('amount', availableBalanceFormatted, true);
    }

    function setRecommendedAmount() {
        setFieldValue('amount', maxSafeAmountFormatted, true);
    }

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
                                    minAmountTxGasBudget
                                        ? `${availableBalanceFormatted} ${symbol} Available`
                                        : '--'
                                }
                                suffix={' ' + symbol}
                                errorMessage={amount && meta.error ? meta.error : undefined}
                                label="Amount"
                                trailingElement={
                                    <ButtonPill onClick={setMaxAmount} disabled={!availableBalance}>
                                        Max
                                    </ButtonPill>
                                }
                            />
                        );
                    }}
                </Field>
                {!isCommitteeMember(validatorAddress) && (
                    <InfoBox
                        type={InfoBoxType.Warning}
                        title="Validator is not earning rewards."
                        supportingText="Validator is active but not in the current committee, so not earning rewards this epoch. It may earn in future epochs. Stake at your discretion."
                        icon={<Warning />}
                        style={InfoBoxStyle.Elevated}
                    />
                )}

                {isUnsafeAmount ? (
                    <InfoBox
                        type={InfoBoxType.Warning}
                        supportingText={
                            <>
                                Staking your full balance may leave you without enough funds to
                                cover gas fees for future actions like unstaking. To avoid this, we
                                recommend staking up to {maxSafeAmountFormatted}&nbsp;
                                {maxSafeAmountSymbol}.
                                <div>
                                    <span
                                        onClick={setRecommendedAmount}
                                        className="cursor-pointer underline hover:opacity-80"
                                    >
                                        Set recommended amount
                                    </span>
                                </div>
                            </>
                        }
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
            {errorMessage ? (
                <div className="my-sm" data-testid="error-info-box">
                    <InfoBox
                        type={InfoBoxType.Error}
                        supportingText={errorMessage}
                        style={InfoBoxStyle.Elevated}
                        icon={<Exclamation />}
                    />
                </div>
            ) : null}
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
