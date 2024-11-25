// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Input, type InputProps } from '@iota/apps-ui-kit';
import { useField } from 'formik';

export function PasswordInputField({ ...props }: InputProps) {
    const [field] = useField(props.name!);
    return (
        <div className="relative flex w-full items-center">
            <Input placeholder="Password" {...props} {...field} />
        </div>
    );
}
