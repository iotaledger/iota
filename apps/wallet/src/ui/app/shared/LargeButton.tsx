// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LoadingIndicator } from '_components';
import clsx from 'clsx';
import { forwardRef, type ReactNode, type Ref } from 'react';

import { ButtonOrLink, type ButtonOrLinkProps } from './utils/ButtonOrLink';

function Decorator({ disabled, children }: { disabled?: boolean; children: ReactNode }) {
    return (
        <div
            className={clsx(
                'flex justify-center bg-transparent text-center text-heading2',
                disabled ? 'text-steel' : 'text-hero-dark group-hover:text-hero',
            )}
        >
            {children}
        </div>
    );
}

interface LargeButtonProps extends ButtonOrLinkProps {
    children: ReactNode;
    loading?: boolean;
    before?: ReactNode;
    after?: ReactNode;
    top?: ReactNode;
    spacing?: string;
    center?: boolean;
    disabled?: boolean;
    primary?: boolean;
}

export const LargeButton = forwardRef(
    (
        {
            top,
            before,
            after,
            center,
            spacing,
            loading,
            disabled,
            children,
            primary,
            className,
            ...otherProps
        }: LargeButtonProps,
        ref: Ref<HTMLAnchorElement | HTMLButtonElement>,
    ) => {
        return (
            <ButtonOrLink
                ref={ref}
                {...otherProps}
                className={clsx(
                    'group flex items-center justify-between rounded-md border border-solid border-transparent px-8 py-2 no-underline',
                    disabled
                        ? 'bg-hero-darkest/5 pointer-events-none'
                        : 'hover:border-iota/10 bg-white/80',
                    primary ? '!bg-iota-primaryBlue2023' : '',
                    spacing === 'sm' && '!p-3',
                    className,
                )}
            >
                {loading && (
                    <div className="flex h-full w-full items-center justify-center p-2">
                        <LoadingIndicator />
                    </div>
                )}
                {!loading && (
                    <div
                        className={clsx(
                            'flex w-full items-center gap-2.5',
                            center && 'justify-center',
                        )}
                    >
                        {before && <Decorator disabled={disabled}>{before}</Decorator>}
                        <div className="flex flex-col">
                            {top && <Decorator disabled={disabled}>{top}</Decorator>}
                            <div
                                className={clsx(
                                    'text-bodySmall font-semibold',
                                    disabled
                                        ? 'text-steel'
                                        : 'text-hero-dark group-hover:text-hero',
                                    primary ? '!text-white' : '',
                                )}
                            >
                                {children}
                            </div>
                        </div>
                        {after && (
                            <div className="ml-auto">
                                <Decorator disabled={disabled}>{after}</Decorator>
                            </div>
                        )}
                    </div>
                )}
            </ButtonOrLink>
        );
    },
);
