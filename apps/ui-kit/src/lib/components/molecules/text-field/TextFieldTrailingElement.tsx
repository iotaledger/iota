// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { TextFieldProps } from './TextField';
import cx from 'classnames';

type TrailingElementProps = Pick<TextFieldProps, 'trailingElement' | 'isContentVisible'>;

type TextFieldTrailingElementProps = TrailingElementProps & {
    onToggleButtonClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    onClearInput?: (e: React.MouseEvent<HTMLButtonElement>) => void;
};

export function TextFieldTrailingElement({
    onClearInput,
    onToggleButtonClick,
    trailingElement,
    isContentVisible,
}: TextFieldTrailingElementProps) {
    if (trailingElement) {
        return trailingElement;
    }

    if (onToggleButtonClick) {
        return (
            <button
                onClick={onToggleButtonClick}
                className={cx('text-neutral-10 dark:text-neutral-92')}
            >
                {isContentVisible ? (
                    <VisibilityOn width={20} height={20} />
                ) : (
                    <VisibilityOff width={20} height={20} />
                )}
            </button>
        );
    }

    if (onClearInput) {
        return (
            <button className="text-neutral-10 dark:text-neutral-92" onClick={onClearInput}>
                <Close />
            </button>
        );
    }
}
