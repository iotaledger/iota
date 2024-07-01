// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Button } from '_app/shared/ButtonUI';
import { useZodForm } from '@iota/core';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';

import { seedValidation } from '../../helpers/validation/seedValidation';
import { Form } from '../../shared/forms/Form';
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
        <Form className="flex h-full flex-col gap-2" form={form} onSubmit={onSubmit}>
            <TextAreaField label="Enter Seed" rows={5} {...register('seed')} />
            <div className="mt-auto flex gap-2.5">
                <Button variant="outline" size="tall" text="Cancel" onClick={() => navigate(-1)} />
                <Button
                    type="submit"
                    disabled={isSubmitting || !isValid}
                    variant="primary"
                    size="tall"
                    loading={isSubmitting}
                    text="Add Account"
                />
            </div>
        </Form>
    );
}
