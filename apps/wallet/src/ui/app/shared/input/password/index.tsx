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
                    'peer flex h-11 w-full items-center gap-5 rounded-2lg border border-solid border-gray-45 bg-white py-2.5 pl-3 pr-0 text-body font-medium text-steel-dark placeholder-gray-65 shadow-button focus:border-steel focus:shadow-none'
                }
            />
            <IconComponent
                className="absolute right-3 cursor-pointer text-heading6 font-normal text-gray-60 peer-focus:text-steel"
                onClick={() => setPasswordShown(!passwordShown)}
            />
        </div>
    );
}
