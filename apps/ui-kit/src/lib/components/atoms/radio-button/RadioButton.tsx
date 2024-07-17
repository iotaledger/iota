// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useEffect, useState } from 'react';
import cx from 'classnames';
import { RadioOn, RadioOff } from '@iota/ui-icons';

interface RadioButtonProps {
    /**
     * The label of the radio button.
     */
    label: string;
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
}

const RadioButton: React.FC<RadioButtonProps> = ({ label, isChecked, isDisabled, onChange }) => {
    const [checked, setChecked] = useState(isChecked);

    // Update local state when isChecked prop changes
    useEffect(() => {
        setChecked(isChecked);
    }, [isChecked]);

    const handleChange = (event: React.ChangeEvent<HTMLInputElement>) => {
        setChecked(event.target.checked);
        if (onChange) {
            onChange(event);
        }
    };

    const RadioIcon = checked ? RadioOn : RadioOff;
    const inputId = `radio-${label}`;

    return (
        <label
            htmlFor={inputId}
            className={cx('group flex cursor-pointer flex-row gap-x-1 text-center', {
                disabled: isDisabled,
            })}
        >
            <div
                className={cx(
                    'relative flex h-[40px] w-[40px] items-center justify-center rounded-full',
                    {
                        'state-layer': !isDisabled,
                    },
                )}
            >
                <input
                    id={inputId}
                    type="radio"
                    checked={checked}
                    onChange={handleChange}
                    disabled={isDisabled}
                    className={cx('peer appearance-none disabled:opacity-40')}
                />
                <span
                    className="absolute
                    text-neutral-40 peer-checked:text-primary-30 peer-disabled:opacity-40 peer-checked:peer-disabled:text-neutral-40 dark:text-neutral-60 dark:peer-checked:peer-disabled:text-neutral-40
                "
                >
                    <RadioIcon width={24} height={24} />
                </span>
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
