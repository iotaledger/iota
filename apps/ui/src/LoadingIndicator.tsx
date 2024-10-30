// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, type VariantProps } from 'class-variance-authority';

const loadingIndicatorStyles = cva('animate-spin text-steel', {
    variants: {
        variant: {
            md: 'h-4 w-4',
            lg: 'h-6 w-6',
        },
    },
    defaultVariants: {
        variant: 'md',
    },
});

type LoadingIndicatorStylesProps = VariantProps<typeof loadingIndicatorStyles>;

export interface LoadingIndicatorProps extends LoadingIndicatorStylesProps {
    text?: string;
}

export function LoadingIndicator({ text, variant }: LoadingIndicatorProps) {
    return (
        <div className="inline-flex flex-row flex-nowrap items-center gap-3">
            {text ? <div className="text-body font-medium text-steel-dark">{text}</div> : null}
        </div>
    );
}