// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useRef } from 'react';
import { ToggleLabelPosition, ToggleSize } from './toggle.enums';
import {
    TOGGLE,
    TOGGLE_LABEL,
    TOGGLE_SIZE,
    TOGGLE_STATES,
    TOGGLE_THUMB,
    TOGGLE_THUMB_COLOR,
    TOGGLE_THUMB_POSITION,
    TOGGLE_THUMB_SIZE,
    TOGGLE_CONTAINER,
} from './toggle.classes';
import cx from 'classnames';

interface ToggleProps {
    /**
     * The label for the toggle.
     */
    label?: string | React.ReactNode;
    /**
     * The state of the toggle (on or off).
     */
    isToggled: boolean;
    /**
     * Whether the label should be placed before the toggle.
     */
    labelPosition?: ToggleLabelPosition;
    /**
     * If true, the toggle will be disabled.
     */
    isDisabled?: boolean;
    /**
     * The callback to call when the toggle state changes.
     */
    onChange?: (isToggled: boolean, event: React.ChangeEvent<HTMLInputElement>) => void;
    /**
     * The name and id of the toggle input.
     */
    name?: string;
    /**
     * The size of the toggle.
     */
    size?: ToggleSize;
    /**
     * The 'data-testid' attribute value (used in e2e tests)
     */
    testId?: string;
}

export const Toggle = forwardRef<HTMLInputElement, ToggleProps>(
    (
        {
            label,
            isToggled,
            labelPosition = ToggleLabelPosition.Right,
            isDisabled = false,
            onChange,
            name,
            size = ToggleSize.Default,
            testId,
        }: ToggleProps,
        ref,
    ) => {
        const inputRef = useRef<HTMLInputElement | null>(null);

        function handleChange(e: React.ChangeEvent<HTMLInputElement>) {
            const newChecked = e.target.checked;
            onChange?.(newChecked, e);
        }

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

        const toggleClasses = cx(TOGGLE, {
            [TOGGLE_STATES.active]: isToggled && !isDisabled,
            [TOGGLE_STATES.inactive]: !isToggled,
            [TOGGLE_STATES.disabledActive]: isDisabled && isToggled,
            [TOGGLE_STATES.disabled]: isDisabled,
            [TOGGLE_SIZE[size]]: true,
        });

        const thumbClasses = cx(TOGGLE_THUMB, {
            [TOGGLE_THUMB_POSITION[size].unchecked]: !isToggled,
            [TOGGLE_THUMB_POSITION[size].checked]: isToggled,
            [TOGGLE_THUMB_COLOR.unchecked]: !isToggled,
            [TOGGLE_THUMB_COLOR.checked]: isToggled,
            [TOGGLE_THUMB_SIZE[size]]: true,
        });

        const containerClasses = cx(TOGGLE_CONTAINER, {
            'flex-row-reverse': labelPosition === ToggleLabelPosition.Left,
            'cursor-not-allowed': isDisabled,
        });
        const labelClasses = cx(TOGGLE_LABEL, {
            'opacity-40': isDisabled && !isToggled,
        });

        return (
            <div className={containerClasses}>
                <input
                    id={name}
                    name={name}
                    type="checkbox"
                    className="sr-only"
                    checked={isToggled}
                    ref={assignRefs}
                    disabled={isDisabled}
                    onChange={handleChange}
                    data-testid={testId}
                />

                <span
                    role="switch"
                    onClick={() => inputRef.current?.click()}
                    className={toggleClasses}
                >
                    <span className={thumbClasses} />
                </span>

                {label && (
                    <label htmlFor={name} className={labelClasses}>
                        {label}
                    </label>
                )}
            </div>
        );
    },
);
