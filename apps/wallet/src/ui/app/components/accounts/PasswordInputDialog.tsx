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
    const [countdownError, setCountdownError] = useState<string | null>(null);
    const [runLockInterval, setRunLockInterval] = useState<boolean>(true);
    const backgroundService = useBackgroundClient();

    // Run the lock interval if the popup just got opened again
    useEffect(() => {
        if (open && !runLockInterval) {
            setRunLockInterval(true);
        }
    }, [open]);

    useEffect(() => {
        if (!open || !runLockInterval) return;

        async function checkLockState() {
            const { remainingTime } = await backgroundService.getLockedState({});

            if (remainingTime <= 0) {
                // It is unlockable now so we cancel the interval and clear the error
                setCountdownError(null);
                setRunLockInterval(false);
            } else {
                // Update the error
                const remainingSeconds = Math.ceil(remainingTime / MILLISECONDS_PER_SECOND);
                const message = `Too many failed attempts. Please try again in ${remainingSeconds} ${remainingSeconds === 1 ? 'second' : 'seconds'}.`;
                setCountdownError(message);
            }
        }

        const interval = setInterval(() => {
            checkLockState();
        }, MILLISECONDS_PER_SECOND);

        checkLockState();

        return () => {
            clearInterval(interval);
        };
    }, [runLockInterval, open]);

    const {
        register,
        setError,
        reset,
        formState: { isSubmitting, isValid },
    } = form;

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
                if (AccountTooManyAttemptsError.is(e)) {
                    setRunLockInterval(true);
                } else {
                    setError('password', { message: e.message }, { shouldFocus: true });
                }
            }
        }
    }

    const isConfirmDisabled =
        !!countdownError || isSubmitting || !isValid || !!form.formState.errors.password?.message;

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
                                        countdownError || form.formState.errors.password?.message
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
                                                className="absolute top-0 text-body-sm text-iota-neutral-40 no-underline dark:text-iota-neutral-60"
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
