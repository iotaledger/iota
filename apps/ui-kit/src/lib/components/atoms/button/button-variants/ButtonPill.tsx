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
            className="button-pill-border-color button-pill-text-color flex items-center justify-center rounded-xl border px-sm text-body-md disabled:opacity-40"
            type={htmlType}
            tabIndex={tabIndex}
            {...buttonProps}
        >
            {children}
        </button>
    );
}
