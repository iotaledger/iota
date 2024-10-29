// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useEffect } from 'react';
import { useFormContext } from 'react-hook-form';
import { z } from 'zod';
import { CheckboxField } from '../../shared/forms/CheckboxField';
import { SelectField } from '../../shared/forms/SelectField';
import { Input, InputType, type SelectOption } from '@iota/apps-ui-kit';

const LOCK_INTERVALS: SelectOption[] = [
    { id: 'day', label: 'Day' },
    { id: 'hour', label: 'Hour' },
    { id: 'minute', label: 'Minute' },
];
const LOCK_INTERVALS_PLURAL: SelectOption[] = [
    { id: 'day', label: 'Days' },
    { id: 'hour', label: 'Hours' },
    { id: 'minute', label: 'Minutes' },
];

export const zodSchema = z.object({
    autoLock: z
        .object({
            enabled: z.boolean(),
            timer: z.coerce.number().int('Only integer numbers allowed'),
            interval: z.enum(['day', 'hour', 'minute']),
        })
        .refine(({ enabled, timer }) => !enabled || timer > 0, {
            message: 'Minimum of 1 minute is allowed',
            path: ['timer'],
        }),
});

interface AutoLockSelectorProps {
    disabled?: boolean;
}

export function AutoLockSelector({ disabled }: AutoLockSelectorProps) {
    const { watch, trigger, register } = useFormContext();
    const timer = watch('autoLock.timer');
    const timerEnabled = watch('autoLock.enabled');
    useEffect(() => {
        const { unsubscribe } = watch((_, { name, type }) => {
            if (name === 'autoLock.enabled' && type === 'change') {
                trigger('autoLock.timer');
            }
        });
        return unsubscribe;
    }, [watch, trigger]);

    return (
        <div className="flex flex-col gap-xs">
            <CheckboxField
                name="autoLock.enabled"
                label="Auto-lock after I'm inactive for"
                disabled={disabled}
            />
            <div className="flex items-start justify-between gap-xs">
                <div className="w-2/3">
                    <Input
                        disabled={disabled || !timerEnabled}
                        type={InputType.Number}
                        {...register('autoLock.timer')}
                        name="autoLock.timer"
                    />
                </div>
                <div className="w-1/3">
                    <SelectField
                        disabled={disabled || !timerEnabled}
                        name="autoLock.interval"
                        options={Number(timer) === 1 ? LOCK_INTERVALS : LOCK_INTERVALS_PLURAL}
                    />
                </div>
            </div>
        </div>
    );
}
