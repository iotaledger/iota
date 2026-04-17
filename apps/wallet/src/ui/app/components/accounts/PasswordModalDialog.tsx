// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { MILLISECONDS_PER_SECOND, Theme, useTheme, useZodForm } from '@iota/core';
import { useEffect, useState } from 'react';
import { v4 as uuidV4 } from 'uuid';
import { z } from 'zod';
import { useBackgroundClient } from '_hooks';
import { Form } from '../../shared/forms/Form';
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
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { AccountTooManyAttemptsError } from '_src/shared/accounts';
import UnlockWallet from '_assets/images/unlock_wallet.png';
import UnlockWalletDarkmode from '_assets/images/unlock_wallet_darkmode.png';
import clsx from 'clsx';

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
    onForgotPassword?: () => void;
    isLoading?: boolean;
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
    onForgotPassword,
    isLoading,
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
    const { theme } = useTheme();

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
        !!countdownError ||
        isSubmitting ||
        isLoading ||
        !isValid ||
        !!form.formState.errors.password?.message;

    const heightClasses = showForgotPassword
        ? 'min-h-[min(600px,90dvh)] max-h-[calc(100dvh-2rem)]'
        : 'min-h-[min(calc(600px-4rem),80vh)] max-h-[calc(100dvh-4rem)]';

    return (
        <Dialog open={open}>
            <DialogContent containerId="overlay-portal-container">
                {!showForgotPassword && <Header title="" onClose={onClose} />}
                <DialogBody
                    className={clsx(
                        'dialog-body-color flex flex-col overflow-y-auto p-md--rs text-body-sm',
                        heightClasses,
                    )}
                >
                    <div
                        className={clsx(
                            'flex min-h-0 flex-1 flex-col items-center',
                            showForgotPassword ? 'pt-2xl' : '',
                        )}
                    >
                        <>
                            <img
                                src={theme === Theme.Dark ? UnlockWalletDarkmode : UnlockWallet}
                                alt="Unlock wallet"
                                height={178}
                                width="auto"
                                className="aspect-[4/3] h-[178px] w-auto object-cover"
                            />
                            <span className="py-xs text-headline-sm  text-iota-neutral-10 dark:text-iota-neutral-92">
                                {title}
                            </span>
                        </>

                        <Form
                            form={form}
                            id={formID}
                            onSubmit={handleOnSubmit}
                            className="flex min-h-0 w-full flex-1 flex-col"
                        >
                            <div className="flex flex-col gap-y-lg">
                                <div className="flex flex-col gap-y-sm">
                                    <Input
                                        autoFocus
                                        type={InputType.Password}
                                        isVisibilityToggleEnabled
                                        placeholder="Password"
                                        errorMessage={
                                            countdownError ||
                                            form.formState.errors.password?.message
                                        }
                                        {...register('password')}
                                        name="password"
                                        data-amp-mask
                                    />
                                    {showForgotPassword && (
                                        <span
                                            onClick={onForgotPassword}
                                            className="cursor-pointer self-center text-body-sm text-iota-neutral-40 underline dark:text-iota-neutral-60"
                                        >
                                            Forgot Password?
                                        </span>
                                    )}
                                </div>
                            </div>

                            <div className="mt-auto flex gap-2.5 pt-xl">
                                {!showForgotPassword && (
                                    <Button
                                        type={ButtonType.Secondary}
                                        text={cancelText}
                                        onClick={onClose}
                                        fullWidth
                                    />
                                )}
                                <Button
                                    htmlType={ButtonHtmlType.Submit}
                                    type={ButtonType.Primary}
                                    disabled={isConfirmDisabled}
                                    text={confirmText}
                                    icon={isLoading || isSubmitting ? <LoadingIndicator /> : null}
                                    iconAfterText
                                    fullWidth
                                />
                            </div>
                        </Form>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
