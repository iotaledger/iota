// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, type ComponentProps, type ReactNode } from 'react';
import { Controller, useFormContext } from 'react-hook-form';
import FormField from './FormField';
import { Checkbox } from '@iota/apps-ui-kit';

type CheckboxFieldProps = {
    name: string;
    label: ReactNode;
} & Omit<ComponentProps<'input'>, 'ref'>;

export const CheckboxField = forwardRef<HTMLInputElement, CheckboxFieldProps>(
    ({ label, name, ...props }, forwardedRef) => {
        const { control } = useFormContext();
        return (
            <Controller
                control={control}
                name={name}
                render={({ field: { onChange, name, value } }) => (
                    <FormField name={name}>
                        <div className="flex justify-start">
                            <Checkbox
                                label={label}
                                onCheckedChange={onChange}
                                name={name}
                                isChecked={value}
                                ref={forwardedRef}
                                {...props}
                            />
                        </div>
                    </FormField>
                )}
            />
        );
    },
);
