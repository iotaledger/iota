// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';
import type { ComponentProps } from 'react';

type InputProps = Omit<ComponentProps<'input'>, 'className'>;

export const Input = forwardRef<HTMLInputElement, InputProps>((props, forwardedRef) => (
    <input
        className="border-gray-45 text-steel-dark placeholder:text-gray-60 hover:border-steel hover:text-steel-darker focus:border-steel focus:text-steel-darker disabled:border-gray-45 disabled:text-gray-60 peer w-full items-center rounded-lg border border-solid bg-white p-3 text-body font-medium shadow-sm transition"
        ref={forwardedRef}
        {...props}
    />
));
