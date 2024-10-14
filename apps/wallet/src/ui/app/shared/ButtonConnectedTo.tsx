// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import clsx from 'clsx';
import { forwardRef, type ComponentProps, type ReactNode } from 'react';

export interface ButtonConnectedToProps
    extends Omit<ComponentProps<'button'>, 'ref' | 'className'> {
    iconBefore?: ReactNode;
    text?: ReactNode;
    iconAfter?: ReactNode;
    truncate?: boolean;
}

export const ButtonConnectedTo = forwardRef<HTMLButtonElement, ButtonConnectedToProps>(
    ({ iconBefore, iconAfter, text, truncate, ...rest }, ref) => {
        return (
            <button
                {...rest}
                ref={ref}
                className="max-w-28 flex cursor-pointer flex-row items-center gap-1 rounded-full bg-neutral-96 p-xxxs hover:bg-neutral-92"
            >
                <div className="flex rounded-full bg-neutral-50 p-xxxs text-neutral-100">
                    {iconBefore}
                </div>
                <div className={clsx('overflow-hidden', truncate && 'truncate')}>{text}</div>
            </button>
        );
    },
);
