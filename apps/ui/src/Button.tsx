// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';
import { ButtonHTMLAttributes, forwardRef, type ReactNode } from 'react';

const buttonStyles = cva(['inline-flex flex-nowrap items-center justify-center gap-2 relative'], {
    variants: {
        variant: {
            primary: 'bg-iota-dark text-iota-light hover:text-white border-none',
            secondary: 'bg-gray-90 text-gray-50 hover:text-white border-none',
            outline:
                'bg-white border border-steel text-steel-dark hover:text-steel-darker hover:border-steel-dark active:text-steel active:border-steel disabled:border-gray-45 disabled:text-steel-dark',
        },
        size: {
            md: 'px-3 py-2 rounded-md text-bodySmall font-semibold',
            lg: 'px-4 py-3 rounded-lg text-body font-semibold',
        },
        loading: {
            true: 'text-transparent',
        },
    },
    defaultVariants: {
        variant: 'primary',
        size: 'md',
    },
});

export interface ButtonProps
    extends VariantProps<typeof buttonStyles>,
        ButtonHTMLAttributes<HTMLButtonElement> {
    before?: ReactNode;
    after?: ReactNode;
    asChild?: boolean;
}

export const Button = forwardRef<HTMLButtonElement, ButtonProps>(
    ({ variant, size, loading, children, before, after, asChild, ...props }, ref) => {
        const Comp = asChild ? Slot : 'button';

        if (asChild && (loading || before || after)) {
            throw new Error(
                'When using `asChild`, you cannot use any of props: `loading, `before`, `after`',
            );
        }

        return (
            <Comp
                ref={ref}
                className={buttonStyles({ variant, size, loading })}
                {...props}
                disabled={props.disabled ?? loading ?? undefined}
            >
                {asChild ? (
                    children
                ) : (
                    <>
                        {before}
                        {children}
                        {after}
                    </>
                )}
            </Comp>
        );
    },
);
