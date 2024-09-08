// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';
import { Controller, useFormContext } from 'react-hook-form';
import { Select, type SelectOption } from '@iota/apps-ui-kit';

interface SelectFieldProps {
    name: string;
    options: SelectOption[];
    disabled?: boolean;
}

export const SelectField = forwardRef<HTMLButtonElement, SelectFieldProps>(
    ({ name, options, ...props }, forwardedRef) => {
        const { control } = useFormContext();
        return (
            <Controller
                control={control}
                name={name}
                render={({ field }) => {
                    return (
                        <Select
                            onValueChange={field.onChange}
                            value={field.value}
                            {...props}
                            ref={forwardedRef}
                            options={options}
                        />
                    );
                }}
            />
        );
    },
);
