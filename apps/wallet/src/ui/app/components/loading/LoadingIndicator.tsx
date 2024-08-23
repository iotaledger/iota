// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Spinner16 } from '@iota/icons';
import { cva, type VariantProps } from 'class-variance-authority';

const styles = cva('', {
    variants: {
        color: {
            inherit: 'text-inherit',
            iota: 'text-iota',
        },
    },
});

export type LoadingIndicatorProps = VariantProps<typeof styles>;

export function LoadingIndicator({ color = 'iota' }: LoadingIndicatorProps) {
    return <Spinner16 className={styles({ className: 'animate-spin', color })} />;
}

export default LoadingIndicator;
