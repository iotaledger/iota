// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Slot } from '@radix-ui/react-slot';
import clsx from 'clsx';
import type { ButtonHTMLAttributes } from 'react';

import { buttonVariants } from './Button.css.js';
import type { ButtonVariants } from './Button.css.js';

type ButtonProps = {
    asChild?: boolean;
    ref?: React.Ref<HTMLButtonElement>;
} & ButtonHTMLAttributes<HTMLButtonElement> &
    ButtonVariants;

const Button = ({ className, variant, size, asChild = false, ref, ...props }: ButtonProps) => {
    const Comp = asChild ? Slot : 'button';
    return (
        <Comp {...props} className={clsx(buttonVariants({ variant, size }), className)} ref={ref} />
    );
};
Button.displayName = 'Button';

export { Button };
