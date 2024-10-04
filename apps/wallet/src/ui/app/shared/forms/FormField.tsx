// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';
import { FormLabel } from './FormLabel';

interface FormFieldProps {
    name: string;
    label?: ReactNode;
    children: ReactNode;
}

export function FormField({ children, label }: FormFieldProps) {
    return (
        <div className="flex w-full flex-col gap-2.5">
            {label ? <FormLabel label={label}>{children}</FormLabel> : children}
        </div>
    );
}

export default FormField;
