// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ToS_LINK } from '_src/shared/constants';
import { useZodForm } from '@iota/core';
import { useEffect } from 'react';
import { type SubmitHandler } from 'react-hook-form';
import { useNavigate } from 'react-router-dom';
import { z } from 'zod';
import zxcvbn from 'zxcvbn';
import { parseAutoLock, useAutoLockMinutes } from '../../hooks/useAutoLockMinutes';
import { CheckboxField } from '../../shared/forms/CheckboxField';
import { Form } from '../../shared/forms/Form';
import { AutoLockSelector, zodSchema } from './AutoLockSelector';
import { Button, ButtonHtmlType, ButtonType, TextField, TextFieldType } from '@iota/apps-ui-kit';

function addDot(str: string | undefined) {
    if (str && !str.endsWith('.')) {
        return `${str}.`;
    }
    return str;
}

const formSchema = z
    .object({
        password: z
            .object({
                input: z
                    .string()
                    .nonempty('Required')
                    .superRefine((val, ctx) => {
                        const {
                            score,
                            feedback: { warning, suggestions },
                        } = zxcvbn(val);
                        if (score <= 2) {
                            ctx.addIssue({
                                code: z.ZodIssueCode.custom,
                                message: `${addDot(warning) || 'Password is not strong enough.'}${
                                    suggestions ? ` ${suggestions.join(' ')}` : ''
                                }`,
                            });
                        }
                    }),
                confirmation: z.string().nonempty('Required'),
            })
            .refine(({ input, confirmation }) => input && confirmation && input === confirmation, {
                path: ['confirmation'],
                message: "Passwords don't match",
            }),
        acceptedTos: z.literal<boolean>(true, {
            errorMap: () => ({ message: 'Please accept Terms of Service to continue' }),
        }),
    })
    .merge(zodSchema);

export type FormValues = z.infer<typeof formSchema>;

interface ProtectAccountFormProps {
    submitButtonText: string;
    cancelButtonText?: string;
    onSubmit: SubmitHandler<FormValues>;
    displayToS?: boolean;
}

export function ProtectAccountForm({
    submitButtonText,
    cancelButtonText,
    onSubmit,
    displayToS,
}: ProtectAccountFormProps) {
    const autoLock = useAutoLockMinutes();
    const form = useZodForm({
        mode: 'all',
        schema: formSchema,
        values: {
            password: { input: '', confirmation: '' },
            acceptedTos: !!displayToS,
            autoLock: parseAutoLock(autoLock.data || null),
        },
    });
    const {
        watch,
        register,
        formState: { isSubmitting, isValid },
        trigger,
        getValues,
    } = form;
    const navigate = useNavigate();
    useEffect(() => {
        const { unsubscribe } = watch((_, { name, type }) => {
            if (
                name === 'password.input' &&
                type === 'change' &&
                getValues('password.confirmation')
            ) {
                trigger('password.confirmation');
            }
        });
        return unsubscribe;
    }, [watch, trigger, getValues]);
    return (
        <Form className="flex h-full flex-col justify-between" form={form} onSubmit={onSubmit}>
            <div className="flex h-full flex-col gap-6">
                <TextField
                    autoFocus
                    type={TextFieldType.Password}
                    isVisibilityToggleEnabled
                    label="Create Password"
                    placeholder="Password"
                    errorMessage={form.formState.errors.password?.input?.message}
                    {...register('password.input')}
                />
                <TextField
                    type={TextFieldType.Password}
                    isVisibilityToggleEnabled
                    label="Confirm Password"
                    placeholder="Password"
                    errorMessage={form.formState.errors.password?.confirmation?.message}
                    {...register('password.confirmation')}
                />
                <AutoLockSelector />
            </div>
            <div className="flex flex-col gap-4 pt-xxxs">
                {displayToS ? null : (
                    <CheckboxField
                        name="acceptedTos"
                        label={
                            <div className="flex items-center gap-x-0.5 whitespace-nowrap">
                                <span>I read and agreed to the</span>
                                <a href={ToS_LINK} className="text-label-lg text-primary-30">
                                    Terms of Services
                                </a>
                            </div>
                        }
                    />
                )}

                <div className="flex flex-row justify-stretch gap-2.5">
                    {cancelButtonText ? (
                        <Button
                            type={ButtonType.Secondary}
                            text={cancelButtonText}
                            onClick={() => navigate(-1)}
                            fullWidth
                        />
                    ) : null}
                    <Button
                        type={ButtonType.Primary}
                        disabled={isSubmitting || !isValid}
                        text={submitButtonText}
                        fullWidth
                        htmlType={ButtonHtmlType.Submit}
                    />
                </div>
            </div>
        </Form>
    );
}
