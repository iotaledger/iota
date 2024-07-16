// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';

type RadioButtonProps = {
    /**
     * The label of the radio button.
     */
    label?: string;
    /**
     * The state of the radio button.
     */
    isChecked?: boolean;
    /**
     * If radio button disabled.
     */
    isDisabled?: boolean;
    /**
     * The callback to call when the radio button is clicked.
     */
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
} & React.InputHTMLAttributes<HTMLInputElement>;

const RadioButton: React.FC<RadioButtonProps> = ({ label, isChecked, isDisabled, onChange }) => {
    return (
        <label className={cx('group flex flex-row gap-x-1 text-center', { disabled: isDisabled })}>
            <div
                className={cx(
                    'relative flex h-5 w-5 items-center justify-center rounded-full p-lg',
                    {
                        'state-layer': !isDisabled,
                    },
                )}
            >
                <input
                    type="radio"
                    checked={isChecked}
                    onChange={onChange}
                    disabled={isDisabled}
                    className={cx(
                        'peer h-5 w-5 shrink-0 appearance-none rounded-full border border-2 border-neutral-40 checked:border-primary-30 disabled:opacity-40 checked:disabled:border-neutral-40 dark:border-neutral-60 dark:checked:border-primary-30 dark:disabled:border-neutral-40',
                    )}
                />
                <span
                    className="
                    absolute
                    h-2 w-2 rounded-full peer-checked:bg-primary-30 peer-disabled:opacity-40 peer-checked:peer-disabled:bg-neutral-40 dark:peer-checked:peer-disabled:bg-neutral-40
                "
                />
            </div>
            {label && (
                <span className="inline-flex items-center justify-center text-label-lg text-neutral-40 group-[.disabled]:text-opacity-40 dark:text-neutral-60 group-[.disabled]:dark:text-opacity-40">
                    {label}
                </span>
            )}
        </label>
    );
};

export { RadioButton };
