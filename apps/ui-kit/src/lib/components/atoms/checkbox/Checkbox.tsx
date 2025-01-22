// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useEffect, useRef } from 'react';
import cx from 'classnames';
import { Dash, Checkmark } from '@iota/apps-ui-icons';

interface CheckboxProps {
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
                className={cx('group inline-flex', isLabelFirst ? 'flex-row-reverse' : 'flex-row', {
                    disabled: isDisabled,
                    'gap-x-2': label,
                })}
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
                />
                <span
                    onClick={() => inputRef.current?.click()}
                    className="peer-enabled:state-layer relative inset-0 flex h-5 w-5 items-center justify-center rounded border border-neutral-80 text-neutral-40 group-[.disabled]:cursor-not-allowed peer-disabled:text-neutral-70 peer-disabled:text-opacity-40 peer-disabled:opacity-40 peer-[&:is(:checked,:indeterminate)]:border-primary-30 peer-[&:is(:checked,:indeterminate)]:bg-primary-30 peer-[&:is(:checked,:indeterminate)]:text-white peer-[&:not(:checked,:indeterminate)]:text-opacity-40 disabled:peer-[&:not(:checked,:indeterminate)]:border-neutral-70 dark:border-neutral-20 dark:text-neutral-60 dark:peer-disabled:text-neutral-30 dark:peer-disabled:text-opacity-40 dark:peer-disabled:peer-[&:not(:checked,:indeterminate)]:border-neutral-30 dark:peer-disabled:peer-[&:is(:checked,:indeterminate)]:bg-neutral-40 peer-disabled:[&:is(:checked,:indeterminate)]:border-neutral-60 peer-disabled:[&:is(:checked,:indeterminate)]:bg-neutral-60 dark:peer-disabled:[&:is(:checked,:indeterminate)]:border-neutral-40 [&_svg]:h-4 [&_svg]:w-4 peer-[&:not(:checked,:indeterminate)]:[&_svg]:hidden"
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
        <label
            htmlFor={name}
            className="text-label-lg text-neutral-40 group-[.disabled]:cursor-not-allowed group-[.disabled]:text-opacity-40 dark:text-neutral-60 group-[.disabled]:dark:text-opacity-40"
        >
            {label}
        </label>
    );
}
