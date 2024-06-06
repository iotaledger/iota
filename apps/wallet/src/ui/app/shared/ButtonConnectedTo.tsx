// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { cva, cx, type VariantProps } from 'class-variance-authority';
import clsx from 'clsx';
import { forwardRef, type ComponentProps, type ReactNode } from 'react';

const styles = cva(
    [
        'cursor-pointer outline-0 flex flex-row items-center py-1 px-2 gap-1 rounded-2xl',
        'transition text-body-small font-medium border border-solid w-full min-w-0',
        'border-1 border-gray-45 bg-transparent group',
        'hover:text-hero hover:bg-iota-light hover:border-iota',
        'focus:text-hero focus:bg-iota-light focus:border-iota',
        'active:text-steel active:bg-gray-45 active:border-transparent',
        'disabled:text-gray-60 disabled:bg-transparent disabled:border-transparent',
    ],
    {
        variants: {
            bgOnHover: {
                blueLight: ['text-hero'],
                grey: ['text-steel-dark'],
            },
        },
        defaultVariants: {
            bgOnHover: 'blueLight',
        },
    },
);

export interface ButtonConnectedToProps
    extends VariantProps<typeof styles>,
        Omit<ComponentProps<'button'>, 'ref' | 'className'> {
    iconBefore?: ReactNode;
    text?: ReactNode;
    iconAfter?: ReactNode;
    truncate?: boolean;
}

export const ButtonConnectedTo = forwardRef<HTMLButtonElement, ButtonConnectedToProps>(
    ({ bgOnHover, iconBefore, iconAfter, text, truncate, ...rest }, ref) => {
        return (
            <button {...rest} ref={ref} className={styles({ bgOnHover })}>
                <div className="flex">{iconBefore}</div>
                <div className={clsx('overflow-hidden', truncate && 'truncate')}>{text}</div>
                <div
                    className={cx(
                        'flex',
                        bgOnHover === 'grey'
                            ? 'group-active::text-inherit text-steel group-hover:text-inherit group-focus:text-inherit'
                            : null,
                    )}
                >
                    {iconAfter}
                </div>
            </button>
        );
    },
);
