// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';

type RadioButtonProps = {
    checked?: boolean;
    disabled?: boolean;
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
} & React.InputHTMLAttributes<HTMLInputElement>;

const RadioButton: React.FC<RadioButtonProps> = ({ checked, disabled, onChange }) => {
    return (
        <div
            className={cx('relative flex h-4 w-4 items-center justify-center rounded-full p-lg', {
                'state-layer': !disabled,
            })}
        >
            <input
                type="radio"
                checked={checked}
                onChange={onChange}
                disabled={disabled}
                className={cx(
                    'peer h-4 w-4 shrink-0 appearance-none rounded-full border border-2 border-neutral-40 checked:border-primary-30 disabled:opacity-40 checked:disabled:border-neutral-40 dark:border-neutral-60 dark:checked:border-primary-30 dark:disabled:border-neutral-40',
                )}
            />
            <span
                className="
                absolute
                h-2 w-2 rounded-full peer-checked:bg-primary-30 peer-disabled:opacity-40 peer-checked:peer-disabled:bg-neutral-40 dark:peer-checked:peer-disabled:bg-neutral-40
            "
            />
        </div>
    );
};

export { RadioButton };
