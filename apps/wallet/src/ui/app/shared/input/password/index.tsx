// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { EyeClose16, EyeOpen16 } from '@iota/icons';
import { useField } from 'formik';
import { useState, type ComponentProps } from 'react';

export interface PasswordInputProps
    extends Omit<ComponentProps<'input'>, 'className' | 'type' | 'name'> {
    name: string;
}

export function PasswordInputField({ ...props }: PasswordInputProps) {
    const [passwordShown, setPasswordShown] = useState(false);
    const [field] = useField(props.name);
    const IconComponent = passwordShown ? EyeOpen16 : EyeClose16;
    return (
        <div className="relative flex w-full items-center">
            <input
                type={passwordShown ? 'text' : 'password'}
                placeholder="Password"
                {...props}
                {...field}
                className={
                    'border-gray-45 text-steel-dark placeholder-gray-65 focus:border-steel peer flex h-11 w-full items-center gap-5 rounded-2lg border border-solid bg-white py-2.5  pl-3 pr-0 text-body font-medium shadow-button focus:shadow-none'
                }
            />
            <IconComponent
                className="text-gray-60 peer-focus:text-steel absolute right-3 cursor-pointer text-heading6 font-normal"
                onClick={() => setPasswordShown(!passwordShown)}
            />
        </div>
    );
}
