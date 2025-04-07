// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { forwardRef, useEffect, useRef, useState } from 'react';
import cx from 'classnames';

interface ToggleProps {
    /**
     * The label for the toggle.
     */
    label?: string | React.ReactNode;
    /**
     * The state of the toggle (on or off).
     */
    isActive?: boolean;
    /**
     * Whether the label should be placed before the toggle.
     */
    labelPosition?: 'left' | 'right';
    /**
     * If true, the toggle will be disabled.
     */
    disabled?: boolean;
    /**
     * The callback to call when the toggle state changes.
     */
    onChange?: (isActive: boolean, event: React.ChangeEvent<HTMLInputElement>) => void;
    /**
     * The name and id of the toggle input.
     */
    name?: string;
    /**
     * The size of the toggle.
     */
    size?: 'sm' | 'md';
    /**
     * Additional class names for the label.
     */
    labelClassName?: string;
}

export const Toggle = forwardRef<HTMLInputElement, ToggleProps>(
    (
        {
            label,
            isActive = false,
            labelPosition = 'right',
            disabled = false,
            onChange,
            name,
            size = 'md',
            labelClassName,
        }: ToggleProps,
        ref,
    ) => {
        const inputRef = useRef<HTMLInputElement | null>(null);
        const [isChecked, setIsChecked] = useState(isActive);

        useEffect(() => {
            setIsChecked(isActive);
        }, [isActive]);

        const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
            const newChecked = e.target.checked;
            setIsChecked(newChecked);
            onChange?.(newChecked, e);
        };

        const toggleClasses = cx(
            'relative inline-flex items-center p-xxs border rounded-full transition-all duration-200 cursor-pointer ',
            {
                'bg-primary-30 border-primary-30': isChecked,
                'bg-primary-100 border-neutral-70': !isChecked,
                'opacity-50 cursor-not-allowed': disabled,
                'h-5 w-8': size === 'sm',
                'h-6 w-12': size === 'md',
            },
        );

        const thumbClasses = cx('inline-block rounded-full transition-all duration-200', {
            'translate-x-0 bg-neutral-60': !isChecked,
            'translate-x-full bg-white ': isChecked,
            'h-2 w-2': size === 'sm',
            'h-4 w-4': size === 'md',
        });

        const containerClasses = cx('inline-flex items-center gap-2', {
            'flex-row-reverse': labelPosition === 'left',
            'cursor-not-allowed': disabled,
        });

        return (
            <div className={containerClasses}>
                <input
                    id={name}
                    name={name}
                    type="checkbox"
                    className="sr-only"
                    checked={isChecked}
                    ref={(el) => {
                        inputRef.current = el;
                        if (typeof ref === 'function') {
                            ref(el);
                        } else if (ref) {
                            ref.current = el;
                        }
                    }}
                    disabled={disabled}
                    onChange={handleChange}
                    aria-checked={isChecked}
                    aria-disabled={disabled}
                />

                <span
                    role="switch"
                    aria-checked={isChecked}
                    onClick={() => inputRef.current?.click()}
                    className={toggleClasses}
                >
                    <span className={thumbClasses} />
                </span>

                {label && (
                    <label
                        htmlFor={name}
                        className={cx(
                            'text-label-lg text-neutral-600 dark:text-neutral-400',
                            {
                                'opacity-40': disabled,
                                'cursor-pointer': !disabled,
                            },
                            labelClassName,
                        )}
                    >
                        {label}
                    </label>
                )}
            </div>
        );
    },
);
