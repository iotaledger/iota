// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, type VariantProps } from 'class-variance-authority';
import type { ReactNode } from 'react';

const cardContentStyle = cva([], {
    variants: {
        variant: {
            white: 'bg-white',
            gray: 'bg-gray-40',
        },
        padding: {
            none: 'p-0',
            small: 'p-3.5',
        },
        titleDivider: {
            true: 'border-t border-t-gray-45 border-solid border-0 border-transparent',
        },
    },
    defaultVariants: {
        variant: 'white',
        padding: 'small',
    },
});

export interface CardProps extends VariantProps<typeof cardContentStyle> {
    header?: ReactNode;
    footer?: ReactNode;
    children?: ReactNode;
}

export function Card({ header, footer, children, ...styleProps }: CardProps) {
    return (
        <div
            className={
                'border-gray-45 box-border flex w-full flex-col overflow-hidden rounded-2xl border border-solid outline-1'
            }
        >
            {header && <div className="bg-gray-40 flex items-center justify-center">{header}</div>}
            <div className={cardContentStyle(styleProps)}>
                {children}
                {footer && (
                    <div className={'flex w-full flex-col justify-center pt-0'}>
                        {children && <span className="bg-gray-45 mb-3.5 h-px w-full px-4"></span>}
                        <div className="flex justify-between">{footer}</div>
                    </div>
                )}
            </div>
        </div>
    );
}

export { CardItem } from './CardItem';
