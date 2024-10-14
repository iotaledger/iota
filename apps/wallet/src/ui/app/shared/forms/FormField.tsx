// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ReactNode } from 'react';

interface FormFieldProps {
    name: string;
    children: ReactNode;
}

export function FormField({ children }: FormFieldProps) {
    return <div className="flex w-full flex-col gap-2.5">{children}</div>;
}

export default FormField;
