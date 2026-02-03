// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ComponentProps, type ReactNode } from 'react';
import { Controller, useFormContext } from 'react-hook-form';
import { Checkbox } from '@iota/apps-ui-kit';

type CheckboxFieldProps = {
    name: string;
    label: ReactNode;
    ref?: React.Ref<HTMLInputElement>;
} & ComponentProps<'input'>;

export function CheckboxField({ label, name, ref, ...props }: CheckboxFieldProps) {
    const { control } = useFormContext();
    return (
        <Controller
            control={control}
            name={name}
            render={({ field: { onChange, name, value } }) => (
                <div className="flex justify-start">
                    <Checkbox
                        label={label}
                        onCheckedChange={onChange}
                        name={name}
                        isChecked={value}
                        ref={ref}
                        {...props}
                        isDisabled={props.disabled}
                    />
                </div>
            )}
        />
    );
}

CheckboxField.displayName = 'CheckboxField';
