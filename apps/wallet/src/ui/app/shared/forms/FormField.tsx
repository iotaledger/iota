// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';
import { useFormContext } from 'react-hook-form';

import Alert from '_app/components/alert';
import { FormLabel } from './FormLabel';

interface FormFieldProps {
    name: string;
    label?: ReactNode;
    children: ReactNode;
}

export function FormField({ children, name, label }: FormFieldProps) {
    const { getFieldState, formState } = useFormContext();
    const state = getFieldState(name, formState);

    return (
        <div className="flex w-full flex-col gap-2.5">
            {label ? <FormLabel label={label}>{children}</FormLabel> : children}
            {state.error && <Alert>{state.error.message}</Alert>}
        </div>
    );
}

export default FormField;
