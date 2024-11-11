// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Input, InputType } from '@iota/apps-ui-kit';
import { Close } from '@iota/ui-icons';
import { useIotaAddressValidation } from '../../hooks';
import React, { useCallback } from 'react';

export interface AddressInputProps {
    field: {
        name: string;
        value: string;
        onChange: (e: React.ChangeEvent<HTMLInputElement>) => void;
        onBlur: (e: React.FocusEvent<HTMLInputElement>) => void;
    };
    form: {
        setFieldValue: (field: string, value: string, shouldValidate?: boolean) => void;
        errors: Record<string, string>;
        touched: Record<string, boolean>;
    };
    disabled?: boolean;
    placeholder?: string;
    label?: string;
}

export function AddressInput({
    field,
    form,
    disabled,
    placeholder = '0x...',
    label = 'Enter Recipient Address',
}: AddressInputProps) {
    const iotaAddressValidation = useIotaAddressValidation();

    const formattedValue = iotaAddressValidation.cast(field.value);

    const handleOnChange = useCallback(
        (e: React.ChangeEvent<HTMLInputElement>) => {
            const address = e.currentTarget.value;
            const validatedValue = iotaAddressValidation.cast(address);
            form.setFieldValue(field.name, validatedValue, true);
        },
        [form, field.name, iotaAddressValidation],
    );

    const clearAddress = useCallback(() => {
        form.setFieldValue(field.name, '');
    }, [form, field.name]);

    const errorMessage = form.touched[field.name] && form.errors[field.name];

    return (
        <Input
            type={InputType.Text}
            disabled={disabled}
            placeholder={placeholder}
            value={formattedValue}
            name={field.name}
            onBlur={field.onBlur}
            label={label}
            onChange={handleOnChange}
            errorMessage={errorMessage as string}
            trailingElement={
                formattedValue ? (
                    <button
                        onClick={clearAddress}
                        type="button"
                        className="flex items-center justify-center"
                    >
                        <Close />
                    </button>
                ) : undefined
            }
        />
    );
}