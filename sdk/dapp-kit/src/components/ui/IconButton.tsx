// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Slot } from '@radix-ui/react-slot';
import clsx from 'clsx';
import type { ButtonHTMLAttributes } from 'react';

import * as styles from './IconButton.css.js';

type IconButtonProps = {
    asChild?: boolean;
    'aria-label': string;
    ref?: React.Ref<HTMLButtonElement>;
} & ButtonHTMLAttributes<HTMLButtonElement>;

const IconButton = ({ className, asChild = false, ref, ...props }: IconButtonProps) => {
    const Comp = asChild ? Slot : 'button';
    return <Comp {...props} className={clsx(styles.container, className)} ref={ref} />;
};
IconButton.displayName = 'IconButton';

export { IconButton };
