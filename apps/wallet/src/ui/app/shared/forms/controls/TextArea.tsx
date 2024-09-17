// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';
import type { ComponentProps } from 'react';

type TextAreaProps = Omit<ComponentProps<'textarea'>, 'className' | 'ref'>;

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>((props, forwardedRef) => (
    <textarea
        className="border-gray-45 text-steel-dark focus:border-steel w-full resize-none rounded-2lg border border-solid p-3 text-body font-medium shadow-button focus:shadow-none"
        ref={forwardedRef}
        {...props}
    />
));
