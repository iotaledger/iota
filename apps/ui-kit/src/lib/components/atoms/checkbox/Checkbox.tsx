// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect, useRef } from 'react';
import cx from 'classnames';

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

    return (
        <label className={cx('group flex flex-row gap-x-2', { disabled: isDisabled })}>
            {isLabelFirst && <Label label={label} />}
            <div className="relative h-5 w-5">
                <input
                    type="checkbox"
                    className="enabled:state-layer peer h-full w-full appearance-none rounded border border-neutral-80 disabled:opacity-40 dark:border-neutral-20 [&:is(:checked,:indeterminate)]:border-primary-30 [&:is(:checked,:indeterminate)]:bg-primary-30 disabled:[&:is(:checked,:indeterminate)]:border-neutral-60 disabled:[&:is(:checked,:indeterminate)]:bg-neutral-60 dark:disabled:[&:is(:checked,:indeterminate)]:border-neutral-40 dark:disabled:[&:is(:checked,:indeterminate)]:bg-neutral-40 disabled:[&:not(:checked,:indeterminate)]:border-neutral-70 dark:disabled:[&:not(:checked,:indeterminate)]:border-neutral-30"
                    checked={isChecked}
                    ref={inputRef}
                    disabled={isDisabled}
                    onChange={(e) => onChange?.(e.target.checked)}
                />
                <span className="absolute inset-0 flex h-full w-full items-center justify-center text-neutral-40 peer-disabled:text-neutral-70 peer-disabled:text-opacity-40 peer-[&:is(:checked,:indeterminate)]:text-white peer-[&:not(:checked,:indeterminate)]:text-opacity-40 dark:text-neutral-60 dark:peer-disabled:text-neutral-30 dark:peer-disabled:text-opacity-40">
                    {isIndeterminate ? <IndeterminateIcon /> : <CheckmarkIcon />}
                </span>
            </div>
            {!isLabelFirst && <Label label={label} />}
        </label>
    );
}

function Label({ label }: Pick<CheckboxProps, 'label'>) {
    return (
        <span className="text-label-lg text-neutral-40 group-[.disabled]:text-opacity-40 dark:text-neutral-60 group-[.disabled]:dark:text-opacity-40">
            {label}
        </span>
    );
}

function IndeterminateIcon() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
        >
            <rect x="5.83325" y="9" width="8.33333" height="2" rx="1" fill="currentColor" />
        </svg>
    );
}

function CheckmarkIcon() {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="20"
            height="20"
            viewBox="0 0 20 20"
            fill="none"
        >
            <path
                fill-rule="evenodd"
                clip-rule="evenodd"
                d="M8.54993 11.538L13.508 6.57981C13.8334 6.25438 14.3578 6.25117 14.6793 6.57263C15.0008 6.8941 14.9976 7.41851 14.6721 7.74394L9.12483 13.2913C8.79939 13.6168 8.27496 13.62 7.95349 13.2985L5.58206 10.9272C5.26059 10.6057 5.2638 10.0813 5.58924 9.75585C5.91469 9.43042 6.43911 9.4272 6.76059 9.74867L8.54993 11.538Z"
                fill="currentColor"
            />
        </svg>
    );
}
