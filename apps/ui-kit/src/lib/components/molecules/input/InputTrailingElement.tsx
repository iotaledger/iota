// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { InputProps } from './Input';
import { UnstyledButton } from '../../atoms';
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
            <UnstyledButton
                onClick={onToggleButtonClick}
                className={cx('text-neutral-10 dark:text-neutral-92')}
            >
                {isContentVisible ? <VisibilityOn /> : <VisibilityOff />}
            </UnstyledButton>
        );
    }

    if (onClearInput) {
        return (
            <UnstyledButton className="text-neutral-10 dark:text-neutral-92" onClick={onClearInput}>
                <Close />
            </UnstyledButton>
        );
    }
}
