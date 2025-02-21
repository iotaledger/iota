// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { z } from 'zod';
import { useZodForm } from '@iota/core';
import { ButtonHtmlType, ButtonPill, Input, InputType } from '@iota/apps-ui-kit';

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
                    <ButtonPill htmlType={ButtonHtmlType.Submit} disabled={!isDirty || !isValid}>
                        Save
                    </ButtonPill>
                }
            />
        </form>
    );
}
