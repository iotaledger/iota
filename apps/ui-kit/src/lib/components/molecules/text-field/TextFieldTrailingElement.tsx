// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Close, VisibilityOff, VisibilityOn } from '@iota/ui-icons';
import { TextFieldType } from './text-field.enums';
import { TextFieldProps } from './TextField';
import cx from 'classnames';

type TrailingElementProps = Pick<
    TextFieldProps,
    'value' | 'type' | 'showHideContentButton' | 'trailingElement'
>;

type TextFieldTrailingElementProps = TrailingElementProps & {
    inputType: string | undefined;
    toggleContentVisibility: () => void;
    onClearInput?: () => void;
};

export function TextFieldTrailingElement({
    value,
    type,
    showHideContentButton,
    onClearInput,
    inputType,
    toggleContentVisibility,
    trailingElement,
}: TextFieldTrailingElementProps) {
    if (trailingElement) {
        return trailingElement;
    }

    if (
        (type === TextFieldType.Password && showHideContentButton) ||
        (type === TextFieldType.TextArea && showHideContentButton)
    ) {
        return (
            <button
                onClick={toggleContentVisibility}
                className={cx('text-neutral-10 dark:text-neutral-92', {
                    'mt-auto': type === TextFieldType.TextArea,
                })}
            >
                {inputType === TextFieldType.Password ? <VisibilityOn /> : <VisibilityOff />}
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
