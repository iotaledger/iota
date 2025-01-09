// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, type ComponentProps, type ReactNode } from 'react';
import { TextArea } from '@iota/apps-ui-kit';

type TextAreaFieldProps = {
    name: string;
    label: ReactNode;
} & ComponentProps<typeof TextArea>;

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextAreaFieldProps>(
    ({ label, ...props }, forwardedRef) => <TextArea {...props} label={label} ref={forwardedRef} />,
);
