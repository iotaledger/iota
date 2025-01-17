// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { RadioOn, RadioOff } from '@iota/apps-ui-icons';

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

function RadioButton({
    label,
    isChecked,
    isDisabled,
    onChange,
}: RadioButtonProps): React.JSX.Element {
    const RadioIcon = isChecked ? RadioOn : RadioOff;

    return (
        <label
            className={cx('group flex flex-row gap-x-1 text-center', {
                disabled: isDisabled,
                'cursor-pointer': !isDisabled,
            })}
        >
            <div
                className={cx('relative flex h-10 w-10 items-center justify-center rounded-full', {
                    'state-layer': !isDisabled,
                })}
            >
                <input
                    type="radio"
                    checked={isChecked}
                    onChange={onChange}
                    disabled={isDisabled}
                    className={cx('peer appearance-none disabled:opacity-40')}
                />
                <span
                    className="absolute
                    text-neutral-40 peer-checked:text-primary-30 peer-disabled:opacity-40 peer-checked:peer-disabled:text-neutral-40 dark:text-neutral-60 dark:peer-checked:peer-disabled:text-neutral-40 [&_svg]:h-6 [&_svg]:w-6
                "
                >
                    <RadioIcon />
                </span>
            </div>
            <span className="inline-flex items-center justify-center text-label-lg text-neutral-40 group-[.disabled]:text-opacity-40 dark:text-neutral-60 group-[.disabled]:dark:text-opacity-40">
                {label}
            </span>
        </label>
    );
}

export { RadioButton };
