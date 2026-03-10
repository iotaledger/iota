// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, VariantProps } from 'class-variance-authority';

export const nftDisplayVariants = cva('', {
    variants: {
        size: {
            medium: 'w-[220px]',
            large: 'w-[440px]',
            full: 'w-full',
        },
    },
    defaultVariants: {
        size: 'medium',
    },
});

export type NftDisplayVariants = VariantProps<typeof nftDisplayVariants>;
