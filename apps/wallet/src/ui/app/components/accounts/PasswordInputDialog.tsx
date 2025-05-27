// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MILLISECONDS_PER_SECOND, useZodForm } from '@iota/core';
import { useEffect, useState } from 'react';
import { v4 as uuidV4 } from 'uuid';
import { z } from 'zod';
import { useAccountSources, useBackgroundClient } from '_hooks';
import { Form } from '../../shared/forms/Form';
import { AccountSourceType } from '_src/background/account-sources/accountSource';
import {
    Button,
    ButtonHtmlType,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    Header,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { Link } from 'react-router-dom';
import { AccountTooManyAttemptsError } from '_src/shared/accounts';

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
        shouldUnregister: true,
    });

    const [countdownError, setCountdownError] = useState<{
        interval: NodeJS.Timeout;
        message: string;
    } | null>(null);

    // Clear the interval and error if the dialog closed but not unmounted
    useEffect(() => {
        if (!open && countdownError?.interval) {
            clearInterval(countdownError.interval);
            setCountdownError(null);
        }
    }, [open]);

    // Clear the interval if the dialog unmounted
    useEffect(() => {
        return () => {
            if (countdownError?.interval) {
                clearInterval(countdownError.interval);
            }
        };
    }, [countdownError?.interval]);

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
            await onSubmit(password);
            reset();
        } catch (e) {
            if (e instanceof Error) {
                const verifyError = AccountTooManyAttemptsError.fromError(e);
                if (verifyError) {
                    let remainingTime = Math.ceil(
                        verifyError.remainingTime / MILLISECONDS_PER_SECOND,
                    );

                    function updateCountdown() {
                        const message = `Too many failed attempts. Please try again in ${remainingTime} ${remainingTime === 1 ? 'second' : 'seconds'}.`;
                        setCountdownError({
                            interval,
                            message,
                        });

                        // Clear the interval and error when its done
                        if (remainingTime === 0) {
                            clearInterval(interval);
                            setCountdownError(null);
                            return;
                        }

                        remainingTime -= 1;
                    }

                    let interval = setInterval(updateCountdown, MILLISECONDS_PER_SECOND);
                    updateCountdown();
                } else {
                    setError('password', { message: e.message }, { shouldFocus: true });
                }
            }
        }
    }

    const isConfirmDisabled = !!countdownError;
    isSubmitting || !isValid || !!form.formState.errors.password?.message;

    return (
        <Dialog open={open}>
            <DialogContent containerId="overlay-portal-container">
                <Header title={title} onClose={onClose} />
                <DialogBody>
                    <Form form={form} id={formID} onSubmit={handleOnSubmit}>
                        <div className="flex flex-col gap-y-lg">
                            <div className="flex flex-col gap-y-sm">
                                <Input
                                    autoFocus
                                    type={InputType.Password}
                                    isVisibilityToggleEnabled
                                    placeholder="Password"
                                    errorMessage={
                                        countdownError?.message ||
                                        form.formState.errors.password?.message
                                    }
                                    {...register('password')}
                                    name="password"
                                />
                                {showForgotPassword && (
                                    <div className="relative p-xs">
                                        {hasAccountsSources ? (
                                            <Link
                                                to="/accounts/forgot-password"
                                                onClick={onClose}
                                                className="absolute top-0 text-body-sm text-neutral-40 no-underline dark:text-neutral-60"
                                            >
                                                Forgot Password?
                                            </Link>
                                        ) : null}
                                    </div>
                                )}
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
                                        disabled={isConfirmDisabled}
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
