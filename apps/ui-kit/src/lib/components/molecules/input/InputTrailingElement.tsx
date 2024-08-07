// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { InputProps } from './Input';
import cx from 'classnames';

type TrailingElementProps = Pick<InputProps, 'trailingElement' | 'isContentVisible'>;

type InputTrailingElementProps = TrailingElementProps & {
    onToggleButtonClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    onClearInput?: (e: React.MouseEvent<HTMLButtonElement>) => void;
};

export function InputTrailingElement({
    onClearInput,
    onToggleButtonClick,
    trailingElement,
    isContentVisible,
}: InputTrailingElementProps) {
    if (trailingElement) {
        return trailingElement;
    }

    if (onToggleButtonClick) {
        return (
            <button
                onClick={onToggleButtonClick}
                className={cx('text-neutral-10 dark:text-neutral-92')}
            >
                {isContentVisible ? <VisibilityOn /> : <VisibilityOff />}
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
