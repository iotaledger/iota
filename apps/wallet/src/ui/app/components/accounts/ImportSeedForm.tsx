// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';
import { seedValidation } from '../../helpers/validation/seedValidation';
import { Form } from '../../shared/forms/Form';
import { Button, ButtonType, ButtonHtmlType } from '@iota/apps-ui-kit';
import { TextAreaField } from '../../shared/forms/TextAreaField';

const formSchema = z.object({
    seed: seedValidation,
});

type FormValues = z.infer<typeof formSchema>;

interface ImportSeedFormProps {
    onSubmit: SubmitHandler<FormValues>;
}

export function ImportSeedForm({ onSubmit }: ImportSeedFormProps) {
    const form = useZodForm({
        mode: 'onTouched',
        schema: formSchema,
    });
    const {
        register,
        formState: { isSubmitting, isValid },
    } = form;
    const navigate = useNavigate();

    return (
        <Form
            className="flex h-full flex-col justify-between gap-2"
            form={form}
            onSubmit={onSubmit}
        >
            <TextAreaField
                label="Enter Seed"
                rows={5}
                {...register('seed')}
                errorMessage={form.formState.errors.seed?.message}
            />
            <div className="flex flex-row justify-stretch gap-2.5">
                <Button
                    type={ButtonType.Secondary}
                    text="Cancel"
                    onClick={() => navigate(-1)}
                    fullWidth
                />
                <Button
                    type={ButtonType.Primary}
                    disabled={isSubmitting || !isValid}
                    text="Add Profile"
                    fullWidth
                    htmlType={ButtonHtmlType.Submit}
                />
            </div>
        </Form>
    );
}
