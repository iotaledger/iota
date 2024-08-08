// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from 'react';
import cx from 'classnames';
import { Dash, Checkmark } from '@iota/ui-icons';

interface CheckboxProps {
    /**
     * The label of the checkbox.
     */
    label?: string;
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
    onChange?: (checked: boolean) => void;
}

export function Checkbox({
    isChecked,
    isIndeterminate,
    label,
    isLabelFirst,
    isDisabled,
    onChange,
}: CheckboxProps) {
    const inputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
        if (inputRef.current) {
            inputRef.current.indeterminate = isIndeterminate ?? false;
        }
    }, [isIndeterminate, inputRef]);

    const CheckmarkIcon = isIndeterminate ? Dash : Checkmark;

    return (
        <label
            className={cx('group inline-flex', isLabelFirst ? 'flex-row-reverse' : 'flex-row', {
                disabled: isDisabled,
                'gap-x-2': label,
            })}
        >
            <div className="relative h-5 w-5">
                <input
                    type="checkbox"
                    className="enabled:state-layer peer h-full w-full appearance-none rounded border border-neutral-80 disabled:opacity-40 dark:border-neutral-20 [&:is(:checked,:indeterminate)]:border-primary-30 [&:is(:checked,:indeterminate)]:bg-primary-30 disabled:[&:is(:checked,:indeterminate)]:border-neutral-60 disabled:[&:is(:checked,:indeterminate)]:bg-neutral-60 dark:disabled:[&:is(:checked,:indeterminate)]:border-neutral-40 dark:disabled:[&:is(:checked,:indeterminate)]:bg-neutral-40 disabled:[&:not(:checked,:indeterminate)]:border-neutral-70 dark:disabled:[&:not(:checked,:indeterminate)]:border-neutral-30"
                    checked={isChecked}
                    ref={inputRef}
                    disabled={isDisabled}
                    onChange={(e) => onChange?.(e.target.checked)}
                />
                <span className="absolute inset-0 flex h-full w-full items-center justify-center text-neutral-40 peer-enabled:cursor-pointer peer-disabled:text-neutral-70 peer-disabled:text-opacity-40 peer-[&:is(:checked,:indeterminate)]:text-white peer-[&:not(:checked,:indeterminate)]:text-opacity-40 dark:text-neutral-60 dark:peer-disabled:text-neutral-30 dark:peer-disabled:text-opacity-40">
                    <CheckmarkIcon width={16} height={16} />
                </span>
            </div>
            <LabelText label={label} />
        </label>
    );
}

function LabelText({ label }: Pick<CheckboxProps, 'label'>) {
    return (
        <span className="text-label-lg text-neutral-40 group-[.disabled]:text-opacity-40 dark:text-neutral-60 group-[.disabled]:dark:text-opacity-40">
            {label}
        </span>
    );
}
