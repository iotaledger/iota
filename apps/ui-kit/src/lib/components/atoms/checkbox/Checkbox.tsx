// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useEffect, useRef } from 'react';
import cx from 'classnames';
import { Dash, Checkmark } from '@iota/apps-ui-icons';

interface CheckboxProps extends React.AriaAttributes {
    /**
     * The label of the checkbox.
     */
    label?: string | React.ReactNode;
    /**
     * The state of the checkbox.
     */
    isChecked?: boolean;
    /**
     * If true the checkbox will override the styles to show an indeterminate state.
     */
    isIndeterminate?: boolean;
    /**
     * Whether the label should be placed before the checkbox.
     */
    isLabelFirst?: boolean;
    /**
     * If true the checkbox will be disabled.
     */
    isDisabled?: boolean;
    /**
     * The callback to call when the checkbox is clicked.
     */
    onCheckedChange?: (e: React.ChangeEvent<HTMLInputElement>) => void;
    /**
     * The name of the checkbox.
     */
    name?: string;
}

export const Checkbox = forwardRef<HTMLInputElement, CheckboxProps>(
    (
        {
            isChecked,
            isIndeterminate,
            label,
            isLabelFirst,
            isDisabled,
            onCheckedChange,
            name,
            ...ariaProps
        }: CheckboxProps,
        ref,
    ) => {
        const inputRef = useRef<HTMLInputElement | null>(null);

        useEffect(() => {
            if (inputRef.current) {
                inputRef.current.indeterminate = isIndeterminate ?? false;
            }
        }, [isIndeterminate, inputRef]);

        const CheckmarkIcon = isIndeterminate ? Dash : Checkmark;

        function assignRefs(element: HTMLInputElement) {
            if (ref) {
                if (typeof ref === 'function') {
                    ref(element);
                } else {
                    ref.current = element;
                }
            }
            inputRef.current = element;
        }

        return (
            <div
                className={cx(
                    'group inline-flex has-[:disabled]:opacity-40',
                    isLabelFirst ? 'flex-row-reverse' : 'flex-row',
                    {
                        disabled: isDisabled,
                        'gap-x-2': label,
                    },
                )}
            >
                <input
                    id={name}
                    name={name}
                    type="checkbox"
                    className="peer hidden appearance-none"
                    checked={isChecked}
                    ref={assignRefs}
                    disabled={isDisabled}
                    onChange={(e) => {
                        onCheckedChange?.(e);
                    }}
                    {...ariaProps}
                />
                <span
                    onClick={() => inputRef.current?.click()}
                    className={cx(
                        'checkbox-base checkbox-state checkbox-icon checkbox-icon-hidden',
                        'checkbox-border-default',
                        'peer-[&:is(:checked,:indeterminate)]:checkbox-border-checked',
                        'peer-[&:is(:checked,:indeterminate)]:checkbox-bg-checked',
                        'peer-[&:is(:checked,:indeterminate)]:checkbox-text-checked',
                        'peer-disabled:peer-[&:not(:checked,:indeterminate)]:checkbox-border-color-disabled',
                        'peer-disabled:peer-[&:is(:checked,:indeterminate)]:checkbox-border-color-disabled-checked',
                        'peer-disabled:peer-[&:is(:checked,:indeterminate)]:checkbox-bg-color-disabled-checked',
                    )}
                >
                    <CheckmarkIcon />
                </span>

                <LabelText label={label} name={name} />
            </div>
        );
    },
);

function LabelText({ label, name }: Pick<CheckboxProps, 'label' | 'name'>) {
    return (
        <label htmlFor={name} className="checkbox-label checkbox-label-disabled">
            {label}
        </label>
    );
}
