// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, type ReactNode } from 'react';
import { Controller, useFormContext } from 'react-hook-form';

import { SelectorField } from '@iota/apps-ui-kit';

interface SelectFieldProps {
    name: string;
    options: string[] | { id: string; label: ReactNode }[];
    disabled?: boolean;
}

export const SelectField = forwardRef<HTMLButtonElement, SelectFieldProps>(
    ({ name, options, ...props }, forwardedRef) => {
        const { control } = useFormContext();
        return (
            <Controller
                control={control}
                name={name}
                render={({ field }) => (
                    <SelectorField
                        onValueChange={field.onChange}
                        ref={forwardedRef}
                        options={options}
                        value={field.value}
                        {...props}
                    />
                )}
            />
        );
    },
);
