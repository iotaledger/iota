// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import { useZodForm } from '@iota/core';
import { Input, InputType } from '@iota/apps-ui-kit';

const CustomRPCSchema = z.object({
    url: z.string().url(),
});

interface CustomRPCInputProps {
    value: string;
    onChange(networkUrl: string): void;
}

export function CustomRPCInput({ value, onChange }: CustomRPCInputProps): JSX.Element {
    const { register, handleSubmit, formState } = useZodForm({
        schema: CustomRPCSchema,
        mode: 'all',
        defaultValues: {
            url: value,
        },
    });

    const { errors, isDirty, isValid } = formState;

    return (
        <form
            onSubmit={handleSubmit((values) => {
                onChange(values.url);
            })}
            className="relative flex items-center rounded-md"
        >
            <Input
                {...register('url')}
                type={InputType.Text}
                errorMessage={errors.url ? 'Invalid URL' : undefined}
                trailingElement={
                    <button
                        disabled={!isDirty || !isValid}
                        type="submit"
                        className="flex items-center justify-center rounded-full bg-gray-90 px-2 py-1 text-captionSmall font-semibold uppercase text-white transition disabled:bg-gray-45 disabled:text-gray-65"
                    >
                        Save
                    </button>
                }
            />
        </form>
    );
}
