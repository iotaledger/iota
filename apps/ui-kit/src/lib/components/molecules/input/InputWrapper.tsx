// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { SecondaryText } from '@/components/atoms/secondary-text';
import { LABEL_CLASSES } from './input.classes';
import { createElement } from 'react';

export enum LabelHtmlTag {
    Label = 'label',
    Div = 'div',
}

export interface InputWrapperProps {
    /**
     * Shows a label with the text above the input.
     */
    label?: string;
    /**
     * Shows a caption with the text below the input.
     */
    caption?: string;
    /**
     * Error Message. Overrides the caption.
     */
    errorMessage?: string;
    /**
     * Amount counter that is shown at the side of the caption text.
     */
    amountCounter?: string | number;
    /**
     * Is the input required
     */
    required?: boolean;
    /**
     * Is the input disabled
     */
    disabled?: boolean;
    /**
     * Use a div as a label instead of a label element
     */
    labelHtmlTag?: LabelHtmlTag;
}

export function InputWrapper({
    label,
    caption,
    disabled,
    errorMessage,
    amountCounter,
    required,
    labelHtmlTag = LabelHtmlTag.Label,
    children,
}: React.PropsWithChildren<InputWrapperProps>) {
    return (
        <div
            className={cx('group flex w-full flex-col gap-y-2', {
                'cursor-not-allowed opacity-40': disabled,
                errored: errorMessage,
                enabled: !disabled,
                required: required,
            })}
        >
            {label ? (
                <LabelWrapper labelHtmlTag={labelHtmlTag}>
                    {label}
                    {children}
                </LabelWrapper>
            ) : (
                children
            )}

            {(errorMessage || caption || amountCounter) && (
                <div
                    className={cx(
                        'flex flex-row items-center',
                        caption || errorMessage ? 'justify-between' : 'justify-end',
                    )}
                >
                    {(errorMessage || caption) && (
                        <SecondaryText hasErrorStyles>{errorMessage || caption}</SecondaryText>
                    )}
                    {amountCounter && <SecondaryText>{amountCounter}</SecondaryText>}
                </div>
            )}
        </div>
    );
}

function LabelWrapper({
    labelHtmlTag,
    children,
}: Required<Pick<InputWrapperProps, 'labelHtmlTag'>> & {
    children: React.ReactNode;
}) {
    return createElement(labelHtmlTag, { className: LABEL_CLASSES }, children);
}
