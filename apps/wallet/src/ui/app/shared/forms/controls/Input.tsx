// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';
import type { ComponentProps } from 'react';

type InputProps = Omit<ComponentProps<'input'>, 'className'>;

export const Input = forwardRef<HTMLInputElement, InputProps>((props, forwardedRef) => (
    <input
        className="peer w-full items-center rounded-lg border border-solid border-gray-45 bg-white p-3 text-body font-medium text-steel-dark shadow-sm transition placeholder:text-gray-60 hover:border-steel hover:text-steel-darker focus:border-steel focus:text-steel-darker disabled:border-gray-45 disabled:text-gray-60"
        ref={forwardedRef}
        {...props}
    />
));
