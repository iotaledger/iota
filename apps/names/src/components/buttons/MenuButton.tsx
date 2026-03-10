// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MoreHoriz } from '@iota/apps-ui-icons';
import { cva, VariantProps } from 'class-variance-authority';
import { forwardRef } from 'react';

type ButtonVariantsType = VariantProps<typeof buttonVariants>;
const buttonVariants = cva('[&>svg]:h-5 [&>svg]:w-5 relative state-layer rounded-full p-xs', {
    variants: {
        variant: {
            primary: 'bg-iota-primary-30',
            ghost: 'bg-transparent text-names-neutral-92/40',
        },
    },
    defaultVariants: {
        variant: 'primary',
    },
});

type MenuButtonProps = Omit<React.ButtonHTMLAttributes<HTMLButtonElement>, 'ref'> &
    ButtonVariantsType;

export const MenuButton = forwardRef<HTMLButtonElement, MenuButtonProps>(
    ({ variant, ...props }, ref) => {
        return (
            <button
                className={buttonVariants({ variant })}
                ref={ref}
                data-testid="menu-button"
                aria-label="More options"
                {...props}
            >
                <MoreHoriz className="w-5 h-5 " />
            </button>
        );
    },
);
