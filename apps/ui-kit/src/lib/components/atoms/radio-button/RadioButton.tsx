// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import cx from 'classnames';

type RadioButtonProps = {
    isChecked?: boolean;
    isDisabled?: boolean;
    onChange?: (event: React.ChangeEvent<HTMLInputElement>) => void;
} & React.InputHTMLAttributes<HTMLInputElement>;

const RadioButton: React.FC<RadioButtonProps> = ({ isChecked, isDisabled, onChange }) => {
    return (
        <div
            className={cx('relative flex h-5 w-5 items-center justify-center rounded-full p-lg', {
                'state-layer': !isDisabled,
            })}
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
    );
};

export { RadioButton };
