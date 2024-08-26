// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useField, useFormikContext } from 'formik';
import { useCallback, useMemo } from 'react';
import type { ChangeEventHandler } from 'react';
import { useIotaAddressValidation } from '@iota/core';
import { Input, InputType } from '@iota/apps-ui-kit';
import { Close } from '@iota/ui-icons';

export interface AddressInputProps {
    disabled?: boolean;
    placeholder?: string;
    name: string;
    label?: string;
}

export function AddressInput({
    disabled: forcedDisabled,
    placeholder = '0x...',
    name = 'to',
    label = 'Enter Recipient Address',
}: AddressInputProps) {
    const [field, meta] = useField(name);

    const { isSubmitting, setFieldValue } = useFormikContext();
    const iotaAddressValidation = useIotaAddressValidation();

    const disabled = forcedDisabled !== undefined ? forcedDisabled : isSubmitting;
    const handleOnChange = useCallback<ChangeEventHandler<HTMLInputElement>>(
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

    return (
        <>
            <Input
                type={InputType.Text}
                disabled={disabled}
                placeholder={placeholder}
                value={formattedValue}
                name={name}
                onBlur={field.onBlur}
                label={label}
                onChange={handleOnChange}
                errorMessage={meta.error}
                trailingElement={
                    <button
                        onClick={clearAddress}
                        type="button"
                        className="flex items-center justify-center"
                    >
                        <Close />
                    </button>
                }
            />
        </>
    );
}
