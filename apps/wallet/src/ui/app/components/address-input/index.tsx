// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Text } from '_app/shared/text';
import Alert from '_src/ui/app/components/alert';
import { useIotaClient } from '@iota/dapp-kit';
import { QrCode, X12 } from '@iota/icons';
import { isValidIotaAddress } from '@iota/iota-sdk/utils';
import { useQuery } from '@tanstack/react-query';
import { cx } from 'class-variance-authority';
import { useField, useFormikContext } from 'formik';
import { useCallback, useMemo } from 'react';
import type { ChangeEventHandler } from 'react';
import TextareaAutosize from 'react-textarea-autosize';

import { useIotaAddressValidation } from './validation';

export interface AddressInputProps {
    disabled?: boolean;
    placeholder?: string;
    name: string;
}

enum RecipientWarningType {
    OBJECT = 'OBJECT',
    EMPTY = 'EMPTY',
}

export function AddressInput({
    disabled: forcedDisabled,
    placeholder = '0x...',
    name = 'to',
}: AddressInputProps) {
    const [field, meta] = useField(name);

    const client = useIotaClient();
    const { data: warningData } = useQuery({
        queryKey: ['address-input-warning', field.value],
        queryFn: async () => {
            // We assume this validation will happen elsewhere:
            if (!isValidIotaAddress(field.value)) {
                return null;
            }

            const object = await client.getObject({ id: field.value });

            if (object && 'data' in object) {
                return RecipientWarningType.OBJECT;
            }

            const [fromAddr, toAddr] = await Promise.all([
                client.queryTransactionBlocks({
                    filter: { FromAddress: field.value },
                    limit: 1,
                }),
                client.queryTransactionBlocks({
                    filter: { ToAddress: field.value },
                    limit: 1,
                }),
            ]);

            if (fromAddr.data?.length === 0 && toAddr.data?.length === 0) {
                return RecipientWarningType.EMPTY;
            }

            return null;
        },
        enabled: !!field.value,
        gcTime: 10 * 1000,
        refetchOnMount: false,
        refetchOnWindowFocus: false,
        refetchInterval: false,
    });

    const { isSubmitting, setFieldValue, isValidating } = useFormikContext();
    const iotaAddressValidation = useIotaAddressValidation();

    const disabled = forcedDisabled !== undefined ? forcedDisabled : isSubmitting;
    const handleOnChange = useCallback<ChangeEventHandler<HTMLTextAreaElement>>(
        (e) => {
            const address = e.currentTarget.value;
            setFieldValue(name, iotaAddressValidation.cast(address));
        },
        [setFieldValue, name, iotaAddressValidation],
    );
    const formattedValue = useMemo(
        () => iotaAddressValidation.cast(field?.value),
        [field?.value, iotaAddressValidation],
    );

    const clearAddress = useCallback(() => {
        setFieldValue('to', '');
    }, [setFieldValue]);

    const hasWarningOrError = meta.touched && (meta.error || warningData) && !isValidating;

    return (
        <>
            <div
                className={cx(
                    'box-border flex h-max w-full overflow-hidden rounded-2lg border border-solid bg-white transition-all focus-within:border-steel',
                    hasWarningOrError ? 'border-issue' : 'border-gray-45',
                )}
            >
                <div className="flex min-h-[42px] w-full items-center py-2 pl-3">
                    <TextareaAutosize
                        data-testid="address-input"
                        maxRows={3}
                        minRows={1}
                        disabled={disabled}
                        placeholder={placeholder}
                        value={formattedValue}
                        onChange={handleOnChange}
                        onBlur={field.onBlur}
                        className={cx(
                            'w-full resize-none border-none bg-white font-mono text-bodySmall font-medium leading-100 placeholder:font-mono placeholder:font-normal placeholder:text-steel-dark',
                            hasWarningOrError ? 'text-issue' : 'text-gray-90',
                        )}
                        name={name}
                    />
                </div>

                <div
                    onClick={clearAddress}
                    className="right-0 ml-4 flex w-11 max-w-[20%] cursor-pointer items-center justify-center bg-gray-40"
                >
                    {meta.touched && field.value ? (
                        <X12 className="h-3 w-3 text-steel-darker" />
                    ) : (
                        <QrCode className="h-5 w-5 text-steel-darker" />
                    )}
                </div>
            </div>

            {field.value && !isValidating ? (
                <div className="mt-2.5 w-full">
                    <Alert
                        noBorder
                        rounded="lg"
                        mode={meta.error || warningData ? 'issue' : 'success'}
                    >
                        {warningData === RecipientWarningType.OBJECT ? (
                            <>
                                <Text variant="pBody" weight="semibold">
                                    This address is an Object
                                </Text>
                                <Text variant="pBodySmall" weight="medium">
                                    Once sent, the funds cannot be recovered. Please make sure you
                                    want to send coins to this address.
                                </Text>
                            </>
                        ) : warningData === RecipientWarningType.EMPTY ? (
                            <>
                                <Text variant="pBody" weight="semibold">
                                    This address has no prior transactions
                                </Text>
                                <Text variant="pBodySmall" weight="medium">
                                    Please make sure you want to send coins to this address.
                                </Text>
                            </>
                        ) : (
                            <Text variant="pBodySmall" weight="medium">
                                {meta.error || 'Valid address'}
                            </Text>
                        )}
                    </Alert>
                </div>
            ) : null}
        </>
    );
}
