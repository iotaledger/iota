// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type ButtonProps } from './Button';
import { ButtonHtmlType } from './button.enums';
import cx from 'classnames';

type ButtonPickedProps = Pick<ButtonProps, 'htmlType'>;
type PropsFromButtonElement = Omit<React.HTMLProps<HTMLButtonElement>, 'type'>;

interface ButtonUnstyledProps extends ButtonPickedProps, PropsFromButtonElement {
    children: React.ReactNode;
}

export function ButtonUnstyled({
    htmlType = ButtonHtmlType.Button,
    children,
    className,
    ...buttonProps
}: ButtonUnstyledProps): React.JSX.Element {
    return (
        <button type={htmlType} {...buttonProps} className={cx('appearance-none', className)}>
            {children}
        </button>
    );
}
