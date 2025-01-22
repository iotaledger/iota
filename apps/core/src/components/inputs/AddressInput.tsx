// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Input, InputType } from '@iota/apps-ui-kit';
import { Close } from '@iota/apps-ui-icons';
import { useIotaAddressValidation } from '../../hooks';
import { useCallback } from 'react';
import { useField, useFormikContext } from 'formik';

export interface AddressInputProps {
    name: string;
    disabled?: boolean;
    placeholder?: string;
    label?: string;
}

export function AddressInput({
    name,
    disabled,
    placeholder = '0x...',
    label = 'Enter Recipient Address',
}: AddressInputProps) {
    const { validateField } = useFormikContext();
    const [field, meta, helpers] = useField<string>(name);
    const iotaAddressValidation = useIotaAddressValidation();

    const formattedValue = iotaAddressValidation.cast(field.value);

    const handleOnChange = useCallback(
        async (e: React.ChangeEvent<HTMLInputElement>) => {
            const address = e.currentTarget.value;
            await helpers.setValue(iotaAddressValidation.cast(address));
            validateField(name);
        },
        [name, iotaAddressValidation],
    );

    const clearAddress = () => {
        helpers.setValue('');
    };

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
            errorMessage={meta.error}
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
