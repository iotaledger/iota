// Copyright (c) Mysten Labs, Inc.
// SPDX-License-Identifier: Apache-2.0

import { EyeClose16, EyeOpen16 } from '@mysten/icons';
import { forwardRef, useState, type ComponentProps } from 'react';

import { ButtonOrLink } from '../../utils/ButtonOrLink';
import { Input } from './Input';

type PasswordInputProps = {
    name: string;
} & Omit<ComponentProps<'input'>, 'className' | 'type' | 'name' | 'ref'>;

export const PasswordInput = forwardRef<HTMLInputElement, PasswordInputProps>(
    ({ placeholder, ...props }, forwardedRef) => {
        const [passwordShown, setPasswordShown] = useState(false);
        const IconComponent = passwordShown ? EyeOpen16 : EyeClose16;

        return (
            <div className="relative flex w-full items-center">
                <Input
                    {...props}
                    type={passwordShown ? 'text' : 'password'}
                    placeholder="Password"
                    ref={forwardedRef}
                />
                <ButtonOrLink
                    tabIndex={-1}
                    className="absolute right-3 flex cursor-pointer appearance-none border-none bg-transparent text-gray-60 peer-focus:text-steel"
                    onClick={() => setPasswordShown((prevState) => !prevState)}
                >
                    <IconComponent className="h-4 w-4" />
                </ButtonOrLink>
            </div>
        );
    },
);
