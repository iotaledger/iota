// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ButtonHtmlType } from '../button.enums';
import type { ButtonVariantProps } from './buttonVariants.types';
import cx from 'classnames';

export function ButtonUnstyled({
    htmlType = ButtonHtmlType.Button,
    children,
    className,
    tabIndex = 0,
    testId,
    ...buttonProps
}: ButtonVariantProps): React.JSX.Element {
    return (
        <button
            type={htmlType}
            {...buttonProps}
            className={cx('appearance-none', className)}
            tabIndex={tabIndex}
            data-testid={testId}
        >
            {children}
        </button>
    );
}
