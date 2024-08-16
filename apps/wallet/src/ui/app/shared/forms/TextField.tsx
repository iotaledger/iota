// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, type ComponentProps, type ReactNode } from 'react';
import FormField from './FormField';
import { Input, InputType } from '@iota/apps-ui-kit';

type TextFieldProps = {
    name: string;
    label?: ReactNode;
} & ComponentProps<'input'>;

export const TextField = forwardRef<HTMLInputElement, TextFieldProps>(
    ({ label, ...props }, forwardedRef) => {
        return (
            <FormField name={props.name} label={label}>
                <Input
                    {...props}
                    type={props.type === 'password' ? InputType.Password : InputType.Text}
                    value={props.value ? String(props.value) : ''}
                    ref={forwardedRef}
                />
            </FormField>
        );
    },
);
