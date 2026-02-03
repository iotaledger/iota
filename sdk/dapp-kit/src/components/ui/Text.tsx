// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Slot } from '@radix-ui/react-slot';
import clsx from 'clsx';

import { textVariants } from './Text.css.js';
import type { TextVariants } from './Text.css.js';

type TextAsChildProps = {
    asChild?: boolean;
    as?: never;
};

type TextDivProps = { as: 'div'; asChild?: never };

type TextProps = (TextAsChildProps | TextDivProps) &
    React.HTMLAttributes<HTMLDivElement> &
    TextVariants & {
        ref?: React.Ref<HTMLDivElement>;
    };

const Text = ({
    children,
    className,
    asChild = false,
    as: Tag = 'div',
    size,
    weight,
    color,
    mono,
    ref,
    ...textProps
}: TextProps) => {
    return (
        <Slot
            {...textProps}
            ref={ref}
            className={clsx(textVariants({ size, weight, color, mono }), className)}
        >
            {asChild ? children : <Tag>{children}</Tag>}
        </Slot>
    );
};
Text.displayName = 'Text';

export { Text };
