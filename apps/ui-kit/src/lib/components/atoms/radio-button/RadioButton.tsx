// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import cx from 'classnames';
import { RadioOn, RadioOff } from '@iota/apps-ui-icons';

interface RadioButtonProps extends React.AriaAttributes {
    /**
     * The id of the radio button.
     */
    id?: string;
    /**
     * The name of the radio button.
     */
    name?: string;
    /**
     * The label of the radio button.
     */
    label: string;
    /**
     * Supporting text for the label.
     */
    supportingLabel?: string;
    /**
     * The body text of the radio button.
     */
    body?: string;
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
    id,
    name,
    label,
    supportingLabel,
    body,
    isChecked,
    isDisabled,
    onChange,
    ...ariaProps
}: RadioButtonProps): React.JSX.Element {
    const RadioIcon = isChecked ? RadioOn : RadioOff;

    return (
        <label
            className={cx(
                'flex flex-row items-center gap-x-1 text-center has-[:disabled]:opacity-40',
                {
                    disabled: isDisabled,
                    'cursor-pointer': !isDisabled,
                },
            )}
        >
            <div
                className={cx(
                    'relative flex h-10 w-10 shrink-0 items-center justify-center rounded-full',
                    {
                        'state-layer-secondary': !isDisabled,
                    },
                )}
            >
                <input
                    id={id}
                    name={name}
                    type="radio"
                    checked={isChecked}
                    onChange={onChange}
                    disabled={isDisabled}
                    className={cx('peer appearance-none')}
                    {...ariaProps}
                />
                <span className="radio-icon-color peer-checked:radio-icon-checked-color peer-checked:peer-disabled:radio-icon-checked-disabled-color absolute size-6">
                    <RadioIcon className="size-full" />
                </span>
            </div>
            <div className="flex flex-col items-start gap-xxs text-start">
                <span className="radio-label-color inline-flex items-center justify-center gap-1 text-label-lg">
                    {label}
                    {supportingLabel && (
                        <span className="text-iota-neutral-60 dark:text-iota-neutral-40">
                            {supportingLabel}
                        </span>
                    )}
                </span>
                {body && (
                    <p className="text-body-sm text-iota-neutral-40 dark:text-iota-neutral-60">
                        {body}
                    </p>
                )}
            </div>
        </label>
    );
}

export { RadioButton };
