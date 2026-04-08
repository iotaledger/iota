// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonUnstyled, Input, InputType } from '@iota/apps-ui-kit';
import { CheckmarkFilled, Close, Loader2 } from '@iota/apps-ui-icons';
import { useField, useFormikContext } from 'formik';
import clsx from 'clsx';
import { formatAddress } from '@iota/iota-sdk/utils';
import { shouldResolveInputAsName } from '../../utils/validation/names';
import { useGetIotaNameRecord } from '../../hooks';
import { useEffect } from 'react';

const ICON_COMMON_CLASSES = 'h-5 w-5 text-iota-primary-30 dark:text-iota-primary-70';

export interface AddressInputProps {
    fieldId: string;
    resolvedNameFieldId: string;
    disabled?: boolean;
    placeholder?: string;
    label?: string;
}

export function AddressInput({
    fieldId,
    resolvedNameFieldId,
    disabled,
    placeholder = '0x...',
    label = 'Enter Recipient Address',
}: AddressInputProps) {
    const { validateField, setFieldValue } = useFormikContext();
    const [field, meta, helpers] = useField<string>(fieldId);
    const [resolvedAddressField] = useField<string>(resolvedNameFieldId);

    const isNameInput = shouldResolveInputAsName(field.value);

    const { data: nameRecord, isLoading: isNameRecordLoading } = useGetIotaNameRecord(
        isNameInput ? field.value : null,
    );

    useEffect(() => {
        const resolvedAddress: string | null | undefined = nameRecord
            ? nameRecord.targetAddress
            : undefined;

        if (resolvedAddressField.value !== resolvedAddress) {
            setFieldValue(resolvedNameFieldId, resolvedAddress);
        }

        validateField(fieldId);
    }, [nameRecord, resolvedNameFieldId, field.value, fieldId, resolvedAddressField.value]);

    async function handleOnChange(e: React.ChangeEvent<HTMLInputElement>) {
        const inputValue = e.currentTarget.value;
        await helpers.setValue(inputValue);
        await validateField(fieldId);
        await helpers.setTouched(true);
    }

    function handleClearAddress() {
        helpers.setValue('');
    }

    return (
        <>
            <Input
                type={InputType.Text}
                disabled={disabled}
                placeholder={placeholder}
                value={field.value}
                name={field.name}
                onBlur={field.onBlur}
                label={label}
                onChange={handleOnChange}
                errorMessage={meta.touched ? meta.error : undefined}
                supportingValue={
                    nameRecord?.targetAddress ? formatAddress(nameRecord.targetAddress) : undefined
                }
                trailingElement={
                    field.value ? (
                        <>
                            <ButtonUnstyled
                                onClick={handleClearAddress}
                                className="flex items-center justify-center"
                            >
                                <Close />
                            </ButtonUnstyled>

                            {isNameInput ? (
                                isNameRecordLoading ? (
                                    <Loader2
                                        className={clsx(ICON_COMMON_CLASSES, 'animate-spin')}
                                    />
                                ) : nameRecord?.targetAddress ? (
                                    <CheckmarkFilled className={clsx(ICON_COMMON_CLASSES)} />
                                ) : null
                            ) : null}
                        </>
                    ) : undefined
                }
            />
        </>
    );
}
