// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    Button,
    ButtonHtmlType,
    ButtonPill,
    ButtonType,
    ButtonUnstyled,
    Input,
    InputType,
    KeyValueInfo,
} from '@iota/apps-ui-kit';
import { useCurrentAccount } from '@iota/dapp-kit';
import { type ComponentProps, forwardRef, useCallback, useEffect } from 'react';
import { type SubmitHandler, useFormContext } from 'react-hook-form';
import { WalletConnectInput } from '..';
import { DepositFormData } from '../../lib/schema/bridgeForm.schema';
import BigNumber from 'bignumber.js';
import { useAccount } from 'wagmi';
import { useBridgeStore } from '../../lib/stores';
import { BridgeFormInputName } from '../../lib/enums';
import { MAX_DEPOSIT_INPUT_LENGTH, PLACEHOLDER_VALUE_DISPLAY } from '../../lib/constants';
import { Loader, SwapAccount } from '@iota/apps-ui-icons';
import { useAvailableBalanceL1 } from '../../hooks/useAvailableBalanceL1';
import { useAvailableBalanceL2 } from '../../hooks/useAvailableBalanceL2';

interface DepositFormProps {
    deposit: () => void;
    isGasEstimationLoading: boolean;
    isTransactionLoading: boolean;
    gasEstimation?: string | null;
    gasEstimationEVM?: string | null;
}
export function DepositForm({
    deposit,
    gasEstimation,
    gasEstimationEVM,
    isTransactionLoading,
    isGasEstimationLoading,
}: DepositFormProps) {
    const layer1Account = useCurrentAccount();
    const layer2Account = useAccount();
    const isLayer1WalletConnected = !!layer1Account?.address;
    const isLayer2WalletConnected = layer2Account.isConnected;

    const toggleBridgeDirection = useBridgeStore((state) => state.toggleBridgeDirection);
    const isFromLayer1 = useBridgeStore((state) => state.isFromLayer1);

    const {
        formattedAvailableBalance: formattedAvailableBalanceL1,
        isLoading: isLoadingBalanceL1,
    } = useAvailableBalanceL1();
    const {
        formattedAvailableBalance: formattedAvailableBalanceL2,
        isLoading: isLoadingBalanceL2,
    } = useAvailableBalanceL2();

    const formattedAvailableBalance = isFromLayer1
        ? formattedAvailableBalanceL1
        : formattedAvailableBalanceL2;
    const isLoadingBalance = isFromLayer1 ? isLoadingBalanceL1 : isLoadingBalanceL2;

    const formMethods = useFormContext<DepositFormData>();

    const {
        trigger,
        register,
        handleSubmit,
        getValues,
        formState: { errors, isValid },
        setValue,
        watch,
    } = formMethods;
    const values = watch();

    const { depositAmount, receivingAddress } = values;

    const isPayingAllBalance = new BigNumber(depositAmount).isEqualTo(
        new BigNumber(formattedAvailableBalance),
    );

    useEffect(() => {
        const isFormIncomplete = Object.values(getValues()).some(
            (value) => value === '' || value === undefined,
        );

        // Trigger manual form revalidation if the available balance loaded after the form is already filled
        if (!isLoadingBalance && !isFormIncomplete) {
            trigger();
        }
    }, [isLoadingBalance, trigger, getValues]);

    useEffect(() => {
        // Reset the amount when the transaction is no longer loading
        if (!isTransactionLoading) {
            setValue(BridgeFormInputName.DepositAmount, '');
        }
    }, [isTransactionLoading]);

    const onSubmit: SubmitHandler<DepositFormData> = useCallback(() => {
        deposit();
    }, [deposit, setValue]);

    const fromAddress = isFromLayer1 ? layer1Account?.address : layer2Account?.address;

    const FROM_LABEL = `From ${isFromLayer1 ? 'IOTA' : 'IOTA EVM'}`;

    function handleMaxAmountClick() {
        if (!formattedAvailableBalance) {
            return;
        }
        setValue(BridgeFormInputName.DepositAmount, formattedAvailableBalance, {
            shouldValidate: true,
        });
    }

    const isMaxButtonDisabled =
        (isFromLayer1 && !isLayer1WalletConnected) ||
        (!isFromLayer1 && !isLayer2WalletConnected) ||
        isPayingAllBalance === true;

    const depositAmountErrorMessage =
        depositAmount !== '' ? errors[BridgeFormInputName.DepositAmount]?.message : undefined;
    const receivingAddressErrorMessage =
        receivingAddress !== '' ? errors[BridgeFormInputName.ReceivingAddress]?.message : undefined;

    const caption =
        formattedAvailableBalance && !isLoadingBalance
            ? `${formattedAvailableBalance} IOTA Available`
            : '--';
    const {
        // eslint-disable-next-line @typescript-eslint/no-unused-vars
        onBlur: _onBlur,
        // eslint-disable-next-line @typescript-eslint/no-unused-vars
        onChange: _onChange,
        ...registerDepositAmount
    } = register(BridgeFormInputName.DepositAmount);
    return (
        <form className="flex flex-col gap-y-md--rs" onSubmit={handleSubmit(onSubmit)}>
            <Input
                label="Amount"
                type={InputType.NumericFormat}
                prefix={isPayingAllBalance ? '~ ' : undefined}
                value={depositAmount}
                errorMessage={depositAmountErrorMessage}
                {...registerDepositAmount}
                data-testid="bridge-amount"
                onValueChange={(values) => {
                    setValue(BridgeFormInputName.DepositAmount, values.value, {
                        shouldValidate: true,
                        shouldTouch: true,
                    });
                }}
                caption={caption}
                maxLength={MAX_DEPOSIT_INPUT_LENGTH}
                trailingElement={
                    <ButtonPill onClick={handleMaxAmountClick} disabled={isMaxButtonDisabled}>
                        Max
                    </ButtonPill>
                }
            />
            <div className="relative flex flex-col gap-y-md--rs">
                {fromAddress ? (
                    <Input
                        type={InputType.Text}
                        label={FROM_LABEL}
                        name="senderAddress"
                        value={fromAddress}
                        key={fromAddress}
                        readOnly
                    />
                ) : (
                    <WalletConnectInput label={FROM_LABEL} isLayer1={isFromLayer1} />
                )}

                <div className="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 z-[1]">
                    <Button
                        type={ButtonType.Primary}
                        icon={<SwapAccount className="rotate-90 -scale-x-100" />}
                        onClick={toggleBridgeDirection}
                        testId="toggle-bridge-direction"
                    />
                </div>

                <DestinationInput
                    type={InputType.Text}
                    errorMessage={receivingAddressErrorMessage}
                    {...register(BridgeFormInputName.ReceivingAddress)}
                    data-testid="receive-address"
                />
            </div>

            <div className="flex flex-col p-md">
                {isFromLayer1 && (
                    <KeyValueInfo
                        fullwidth
                        keyText="Est. IOTA Gas Fees"
                        supportingLabel="IOTA"
                        value={gasEstimation ?? PLACEHOLDER_VALUE_DISPLAY}
                    />
                )}
                <KeyValueInfo
                    fullwidth
                    keyText="Est. IOTA EVM Gas fees"
                    supportingLabel="IOTA"
                    value={gasEstimationEVM ?? PLACEHOLDER_VALUE_DISPLAY}
                />
            </div>

            <Button
                text="Bridge Assets"
                htmlType={ButtonHtmlType.Submit}
                disabled={
                    (isFromLayer1 && !isLayer1WalletConnected) ||
                    (!isFromLayer1 && !isLayer2WalletConnected) ||
                    !isValid ||
                    !!Object.values(values).some((value) => value === '') ||
                    isTransactionLoading ||
                    isGasEstimationLoading ||
                    isLoadingBalance
                }
                icon={
                    depositAmount && isTransactionLoading ? (
                        <Loader className="animate-spin" />
                    ) : undefined
                }
                iconAfterText
            />
        </form>
    );
}

type InputProps = Omit<ComponentProps<typeof Input>, 'label' | 'caption' | 'required'>;
const DestinationInput = forwardRef<HTMLInputElement, InputProps>(function DestinationInput(
    { ...props },
    ref,
) {
    const { setValue } = useFormContext<DepositFormData>();
    const layer1Account = useCurrentAccount();
    const layer2Account = useAccount();

    const isFromLayer1 = useBridgeStore((state) => state.isFromLayer1);
    const toggleIsDepositAddressManualInput = useBridgeStore(
        (state) => state.toggleIsDepositAddressManualInput,
    );
    const isManualInput = useBridgeStore((state) => state.isDepositAddressManualInput);

    const isLayer1WalletConnected = !!layer1Account?.address;
    const isLayer2WalletConnected = layer2Account.isConnected;

    const shouldRenderConnectedAddress =
        (isFromLayer1 && isLayer2WalletConnected) || (!isFromLayer1 && isLayer1WalletConnected);

    useEffect(() => {
        const destinationAddress = isFromLayer1 ? layer2Account?.address : layer1Account?.address;

        if (!isManualInput) {
            setValue(BridgeFormInputName.ReceivingAddress, destinationAddress ?? '', {
                shouldValidate: !!destinationAddress,
            });
        } else {
            setValue(BridgeFormInputName.ReceivingAddress, '');
        }
    }, [isManualInput, layer1Account, layer2Account, isFromLayer1, setValue]);

    const TO_LABEL = `To ${isFromLayer1 ? 'IOTA EVM' : 'IOTA'}`;

    return (
        <div className="relative">
            {isManualInput || shouldRenderConnectedAddress ? (
                <Input ref={ref} {...props} caption={TO_LABEL} readOnly={!isManualInput} />
            ) : (
                <WalletConnectInput label={TO_LABEL} isDestination isLayer1={!isFromLayer1} />
            )}

            <ButtonUnstyled
                onClick={toggleIsDepositAddressManualInput}
                className="absolute bottom-0 right-0 group state-layer flex items-center px-xs rounded-full"
                testId="toggle-receiver-address-input"
            >
                <span className="text-body-md text-neutral-40 dark:text-neutral-60">
                    {isManualInput ? 'Connect a wallet' : 'Enter manually'}
                </span>
            </ButtonUnstyled>
        </div>
    );
});
