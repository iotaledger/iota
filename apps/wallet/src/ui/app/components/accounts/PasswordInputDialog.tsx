// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm } from '@iota/core';
import { useState } from 'react';
import { toast } from 'react-hot-toast';
import { v4 as uuidV4 } from 'uuid';
import { z } from 'zod';
import { useAccountSources } from '../../hooks/useAccountSources';
import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { Form } from '../../shared/forms/Form';
import { AccountSourceType } from '_src/background/account-sources/AccountSource';
import {
    Button,
    ButtonHtmlType,
    ButtonType,
    Dialog,
    DialogBody,
    DialogTitle,
    DialogContent,
    Header,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { Link } from 'react-router-dom';

const formSchema = z.object({
    password: z.string().nonempty('Required'),
});

export interface PasswordModalDialogProps {
    onClose: () => void;
    open: boolean;
    showForgotPassword?: boolean;
    title: string;
    confirmText: string;
    cancelText: string;
    onSubmit: (password: string) => Promise<void> | void;
    verify?: boolean;
}

export function PasswordModalDialog({
    onClose,
    onSubmit,
    open,
    verify,
    showForgotPassword,
    title,
    confirmText,
    cancelText,
}: PasswordModalDialogProps) {
    const form = useZodForm({
        mode: 'onChange',
        schema: formSchema,
        defaultValues: {
            password: '',
        },
    });
    const {
        register,
        setError,
        reset,
        formState: { isSubmitting, isValid },
    } = form;
    const backgroundService = useBackgroundClient();
    const [formID] = useState(() => uuidV4());
    const { data: allAccountsSources } = useAccountSources();
    const hasAccountsSources =
        allAccountsSources?.some(
            ({ type }) => type === AccountSourceType.Mnemonic || type === AccountSourceType.Seed,
        ) || false;

    async function handleOnSubmit({ password }: { password: string }) {
        try {
            if (verify) {
                await backgroundService.verifyPassword({ password });
            }
            try {
                await onSubmit(password);
                reset();
            } catch (e) {
                toast.error((e as Error).message || 'Something went wrong');
            }
        } catch (e) {
            setError(
                'password',
                { message: (e as Error).message || 'Wrong password' },
                { shouldFocus: true },
            );
        }
    }

    return (
        <Dialog open={open}>
            <DialogContent containerId="overlay-portal-container" aria-describedby={undefined}>
                <DialogTitle>
                    <Header title={title} onClose={onClose} />
                </DialogTitle>
                <DialogBody>
                    <Form form={form} id={formID} onSubmit={handleOnSubmit}>
                        <div className="flex flex-col gap-y-6">
                            <div className="flex flex-col gap-y-3">
                                <Input
                                    autoFocus
                                    type={InputType.Password}
                                    isVisibilityToggleEnabled
                                    placeholder="Password"
                                    errorMessage={form.formState.errors.password?.message}
                                    {...register('password')}
                                    name="password"
                                />

                                {showForgotPassword && hasAccountsSources ? (
                                    <Link
                                        to="/accounts/forgot-password"
                                        onClick={onClose}
                                        className="text-body-sm text-neutral-40 no-underline"
                                    >
                                        Forgot Password?
                                    </Link>
                                ) : null}
                            </div>
                            <div className="flex flex-col gap-3">
                                <div className="flex gap-2.5">
                                    <Button
                                        type={ButtonType.Secondary}
                                        text={cancelText}
                                        onClick={onClose}
                                        fullWidth
                                    />
                                    <Button
                                        htmlType={ButtonHtmlType.Submit}
                                        type={ButtonType.Primary}
                                        disabled={isSubmitting || !isValid}
                                        text={confirmText}
                                        fullWidth
                                    />
                                </div>
                            </div>
                        </div>
                    </Form>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
