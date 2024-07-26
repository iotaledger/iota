// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { TextFieldType } from './text-field.enums';
import { TextFieldProps } from './TextField';
import cx from 'classnames';

type TrailingElementProps = Pick<
    TextFieldProps,
    'value' | 'type' | 'isToggleButtonVisible' | 'trailingElement' | 'isContentVisible'
>;

type TextFieldTrailingElementProps = TrailingElementProps & {
    inputType: string | undefined;
    onToggleButtonClick?: (e: React.MouseEvent<HTMLButtonElement>) => void;
    onClearInput?: (e: React.MouseEvent<HTMLButtonElement>) => void;
};

export function TextFieldTrailingElement({
    value,
    type,
    isToggleButtonVisible,
    onClearInput,
    inputType,
    onToggleButtonClick,
    trailingElement,
    isContentVisible,
}: TextFieldTrailingElementProps) {
    if (trailingElement) {
        return trailingElement;
    }

    if (
        (type === TextFieldType.Password && isToggleButtonVisible) ||
        (type === TextFieldType.TextArea && isToggleButtonVisible)
    ) {
        return (
            <button
                onClick={onToggleButtonClick}
                className={cx('text-neutral-10 dark:text-neutral-92', {
                    'mt-auto': type === TextFieldType.TextArea,
                })}
            >
                {inputType === TextFieldType.Password && isContentVisible ? (
                    <VisibilityOn />
                ) : (
                    <VisibilityOff />
                )}
            </button>
        );
    }

    if (type === TextFieldType.Text && value) {
        return (
            <button className="text-neutral-10 dark:text-neutral-92" onClick={onClearInput}>
                <Close />
            </button>
        );
    }
}
