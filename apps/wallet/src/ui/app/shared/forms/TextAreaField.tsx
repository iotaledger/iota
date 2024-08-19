// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, type ComponentProps, type ReactNode } from 'react';

import FormField from './FormField';
import { TextArea } from '@iota/apps-ui-kit';

type TextAreaFieldProps = {
    name: string;
    label: ReactNode;
} & ComponentProps<typeof TextArea>;

export const TextAreaField = forwardRef<HTMLTextAreaElement, TextAreaFieldProps>(
    ({ label, ...props }, forwardedRef) => (
        <FormField name={props.name}>
            <TextArea {...props} label={label} ref={forwardedRef} />
        </FormField>
    ),
);
