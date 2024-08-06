// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { SecondaryText } from '../../atoms/secondary-text';
import { LABEL_CLASSES } from './input.classes';

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
    useDivAsLabel?: boolean;
}

export function InputWrapper({
    label,
    caption,
    disabled,
    errorMessage,
    amountCounter,
    required,
    useDivAsLabel,
    children,
}: React.PropsWithChildren<InputWrapperProps>) {
    return (
        <div
            className={cx('group flex flex-col gap-y-2', {
                'opacity-40': disabled,
                errored: errorMessage,
                enabled: !disabled,
                required: required,
            })}
        >
            {label ? (
                <LabelOrDiv useDivAsLabel={useDivAsLabel}>
                    {label}
                    {children}
                </LabelOrDiv>
            ) : (
                children
            )}

            <div
                className={cx(
                    'flex flex-row items-center',
                    caption || errorMessage ? 'justify-between' : 'justify-end',
                )}
            >
                {(errorMessage || caption) && (
                    <SecondaryText>{errorMessage || caption}</SecondaryText>
                )}
                {amountCounter && <SecondaryText>{amountCounter}</SecondaryText>}
            </div>
        </div>
    );
}

function LabelOrDiv({
    useDivAsLabel,
    children,
}: {
    useDivAsLabel?: boolean;
    children: React.ReactNode;
}) {
    if (useDivAsLabel) {
        return <div className={LABEL_CLASSES}>{children}</div>;
    } else {
        return <label className={LABEL_CLASSES}>{children}</label>;
    }
}
