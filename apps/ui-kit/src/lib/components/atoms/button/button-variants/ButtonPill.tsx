// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonHtmlType } from '../button.enums';
import type { ButtonVariantProps } from './buttonVariants.types';

export function ButtonPill({
    htmlType = ButtonHtmlType.Button,
    children,
    tabIndex = 0,
    ...buttonProps
}: Omit<ButtonVariantProps, 'className'>) {
    return (
        <button
            className="flex items-center justify-center rounded-xl border border-neutral-70 px-sm text-body-md text-neutral-40 disabled:opacity-40 dark:border-neutral-40 dark:text-neutral-60"
            type={htmlType}
            tabIndex={tabIndex}
            {...buttonProps}
        >
            {children}
        </button>
    );
}
