// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { SecondaryText } from '../../atoms/secondary-text';

export interface TextFieldWrapperProps {
    /**
     * Shows a label with the text above the input field.
     */
    label?: string;
    /**
     * Shows a caption with the text below the input field.
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
     * Is the input field required
     */
    required?: boolean;
    /**
     * Is the input field disabled
     */
    disabled?: boolean;
    /**
     * Use a div as a label instead of a label element
     */
    useDivAsLabel?: boolean;
}

export function TextFieldWrapper({
    label,
    caption,
    disabled,
    errorMessage,
    amountCounter,
    required,
    useDivAsLabel,
    children,
}: React.PropsWithChildren<TextFieldWrapperProps>) {
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

const LABEL_CLASSES = 'flex flex-col gap-y-2 text-label-lg text-neutral-40 dark:text-neutral-60';
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
