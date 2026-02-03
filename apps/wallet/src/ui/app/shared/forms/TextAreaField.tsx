// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ComponentProps, type ReactNode } from 'react';
import { TextArea } from '@iota/apps-ui-kit';

type TextAreaFieldProps = {
    name: string;
    label: ReactNode;
    ref?: React.Ref<HTMLTextAreaElement>;
} & ComponentProps<typeof TextArea>;

export function TextAreaField({ label, ref, ...props }: TextAreaFieldProps) {
    return <TextArea {...props} label={label} ref={ref} />;
}

TextAreaField.displayName = 'TextAreaField';
