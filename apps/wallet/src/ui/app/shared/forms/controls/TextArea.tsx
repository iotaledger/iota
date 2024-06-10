// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef } from 'react';
import type { ComponentProps } from 'react';

type TextAreaProps = Omit<ComponentProps<'textarea'>, 'className' | 'ref'>;

export const TextArea = forwardRef<HTMLTextAreaElement, TextAreaProps>((props, forwardedRef) => (
    <textarea
        className="w-full resize-none rounded-2lg border border-solid border-gray-45 p-3 text-body font-medium text-steel-dark shadow-button focus:border-steel focus:shadow-none"
        ref={forwardedRef}
        {...props}
    />
));
