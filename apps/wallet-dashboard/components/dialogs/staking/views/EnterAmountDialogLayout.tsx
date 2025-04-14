// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { CoinFormat, useFormatCoin, useStakeTxnInfo, Validator } from '@iota/core';
import {
    Button,
    ButtonType,
    KeyValueInfo,
    Panel,
    Divider,
    Input,
    InputType,
    Header,
    InfoBoxType,
    InfoBoxStyle,
    InfoBox,
} from '@iota/apps-ui-kit';
import { Field, type FieldProps, useFormikContext } from 'formik';
import { Exclamation, Loader } from '@iota/apps-ui-icons';
import { StakedInfo } from './StakedInfo';
import { DialogLayout, DialogLayoutBody, DialogLayoutFooter } from '../../layout';
import { useIotaClientQuery } from '@iota/dapp-kit';
import React from 'react';

interface FormValues {
    amount: string;
}

interface EnterAmountDialogLayoutProps {
    selectedValidator: string;
    senderAddress: string;
    caption: string;
    renderInfo?: React.JSX.Element;
    isLoading: boolean;
    onBack: () => void;
    handleClose: () => void;
    handleStake: () => void;
    isStakeDisabled?: boolean;
    totalGas?: string | number | null;
    renderInputAction?: React.JSX.Element;
    errorMessage?: string;
}

export function EnterAmountDialogLayout({
    selectedValidator,
    totalGas,
    senderAddress,
    caption,
    renderInfo,
    isLoading,
    isStakeDisabled,
    errorMessage,
    onBack,
    handleClose,
    handleStake,
    renderInputAction,
}: EnterAmountDialogLayoutProps): JSX.Element {
    const { data: system } = useIotaClientQuery('getLatestIotaSystemState');
    const { values, errors } = useFormikContext<FormValues>();
    const amount = values.amount;

    const [gas, symbol] = useFormatCoin({ balance: totalGas ?? 0, format: CoinFormat.FULL });

    const { stakedRewardsStartEpoch, timeBeforeStakeRewardsRedeemableAgoDisplay } = useStakeTxnInfo(
        system?.epoch,
    );

    return (
        <DialogLayout>
            <Header title="Enter amount" onClose={handleClose} onBack={onBack} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col justify-between">
                    <div>
                        <div className="mb-md">
                            <Validator address={selectedValidator} isSelected showApy={false} />
                        </div>
                        <StakedInfo
                            validatorAddress={selectedValidator}
                            accountAddress={senderAddress}
                        />
                        <div className="my-md w-full">
                            <Field name="amount">
                                {({
                                    field: { onChange, ...field },
                                    form: { setFieldValue },
                                    meta,
                                }: FieldProps<FormValues>) => {
                                    return (
                                        <Input
                                            {...field}
                                            onValueChange={({ value }) => {
                                                setFieldValue('amount', value, true);
                                            }}
                                            type={InputType.NumericFormat}
                                            label="Amount"
                                            value={amount}
                                            suffix={` ${symbol}`}
                                            placeholder="Enter amount to stake"
                                            errorMessage={
                                                values.amount && meta.error ? meta.error : undefined
                                            }
                                            caption={caption}
                                            trailingElement={renderInputAction}
                                        />
                                    );
                                }}
                            </Field>
                            {renderInfo ? <div className="mt-md">{renderInfo}</div> : null}
                        </div>

                        <Panel hasBorder>
                            <div className="flex flex-col gap-y-sm p-md">
                                <KeyValueInfo
                                    keyText="Staking Rewards Start"
                                    value={stakedRewardsStartEpoch}
                                    fullwidth
                                />
                                <KeyValueInfo
                                    keyText="Redeem Rewards"
                                    value={timeBeforeStakeRewardsRedeemableAgoDisplay}
                                    fullwidth
                                />
                                <Divider />
                                <KeyValueInfo
                                    keyText="Gas fee"
                                    value={gas || '--'}
                                    supportingLabel={symbol}
                                    fullwidth
                                />
                            </div>
                        </Panel>
                    </div>
                </div>
            </DialogLayoutBody>
            <DialogLayoutFooter>
                {errorMessage ? (
                    <div className="mb-sm">
                        <InfoBox
                            type={InfoBoxType.Error}
                            supportingText={errorMessage}
                            style={InfoBoxStyle.Elevated}
                            icon={<Exclamation />}
                        />
                    </div>
                ) : null}
                <div className="flex w-full justify-between gap-sm">
                    <Button fullWidth type={ButtonType.Secondary} onClick={onBack} text="Back" />
                    <Button
                        fullWidth
                        type={ButtonType.Primary}
                        disabled={
                            !amount ||
                            !!errors?.amount ||
                            isLoading ||
                            isStakeDisabled ||
                            !!errorMessage
                        }
                        onClick={handleStake}
                        text="Stake"
                        icon={
                            isLoading ? (
                                <Loader className="animate-spin" data-testid="loading-indicator" />
                            ) : null
                        }
                        iconAfterText
                    />
                </div>
            </DialogLayoutFooter>
        </DialogLayout>
    );
}
