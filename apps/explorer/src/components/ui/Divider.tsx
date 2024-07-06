// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, type VariantProps } from 'class-variance-authority';

const dividerStyles = cva('', {
    variants: {
        vertical: {
            true: 'border-l',
            false: 'grow border-b',
        },
        color: {
            gray45: 'border-gray-45',
            gray40: 'border-gray-40',
            'hero/10': 'border-hero/10',
        },
    },
    defaultVariants: {
        vertical: false,
        color: 'gray45',
    },
});

export type DividerProps = VariantProps<typeof dividerStyles>;

export function Divider({ vertical, color }: DividerProps): JSX.Element {
    return <div className={dividerStyles({ vertical, color })} />;
}
