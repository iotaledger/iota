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
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Input,
    InputType,
} from '@iota/apps-ui-kit';
import { Warning } from '@iota/apps-ui-icons';
import { Link } from 'react-router-dom';

const MAX_UNLOCK_ATTEMPTS = 3;
const WALLET_LOCK_DURATION_IN_MS = 60000; // 60 seconds

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
    const [failedAttempts, setFailedAttempts] = useState<number>(0);
    const [isLockedOut, setIsLockedOut] = useState<boolean>(false);
    const [remainingLockTime, setRemainingLockTime] = useState<number>(0);

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

    useEffect(() => {
        const syncLockState = async () => {
            const { lockTimeMs, isLockedOut } =
                await backgroundService.getStateAfterManyFailedAttempts();
            if (isLockedOut && lockTimeMs) {
                const elapsedTime = Date.now() - lockTimeMs;
                const remainingTime = Math.max(0, WALLET_LOCK_DURATION_IN_MS - elapsedTime);

                if (remainingTime > 0) {
                    setIsLockedOut(true);
                    setRemainingLockTime(Math.ceil(remainingTime / MILLISECONDS_PER_SECOND));
                } else {
                    await backgroundService.clearStateAfterManyFailedAttempts();
                    setIsLockedOut(false);
                    setRemainingLockTime(0);
                }
            }
        };

        if (open) {
            syncLockState();
        }
    }, [open, backgroundService]);

    useEffect(() => {
        if (!isLockedOut || remainingLockTime <= 0) return;

        const timer = setInterval(() => {
            setRemainingLockTime((prev) => {
                const timeLeft = prev - 1;
                if (timeLeft <= 0) {
                    setIsLockedOut(false);
                    backgroundService.clearStateAfterManyFailedAttempts();
                    clearInterval(timer);
                    return 0;
                }
                return timeLeft;
            });
        }, MILLISECONDS_PER_SECOND);

        return () => clearInterval(timer);
    }, [isLockedOut, remainingLockTime, backgroundService]);

    async function handleOnSubmit({ password }: { password: string }) {
        if (isLockedOut) return;
        try {
            if (verify) {
                await backgroundService.verifyPassword({ password });
            }
            await onSubmit(password);
            reset();
            setFailedAttempts(0);
        } catch (e) {
            const attempts = failedAttempts + 1;
            setFailedAttempts(attempts);
            if (attempts >= MAX_UNLOCK_ATTEMPTS) {
                const lockStartTime = Date.now();
                await backgroundService.setStateAfterManyFailedAttempts(lockStartTime, true);
                setIsLockedOut(true);
                setRemainingLockTime(WALLET_LOCK_DURATION_IN_MS / MILLISECONDS_PER_SECOND);
            }
            setError(
                'password',
                { message: (e as Error).message || 'Wrong password' },
                { shouldFocus: true },
            );
        }
    }

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
                                    errorMessage={form.formState.errors.password?.message}
                                    {...register('password')}
                                    name="password"
                                />
                                {isLockedOut && remainingLockTime > 0 && (
                                    <InfoBox
                                        title="Too many attempts"
                                        icon={<Warning />}
                                        type={InfoBoxType.Warning}
                                        style={InfoBoxStyle.Elevated}
                                        supportingText={`You can try again in ${remainingLockTime} seconds`}
                                    />
                                )}
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
                                        disabled={
                                            isSubmitting ||
                                            !isValid ||
                                            (isLockedOut && remainingLockTime > 0)
                                        }
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
