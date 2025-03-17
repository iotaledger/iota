// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useZodForm, toast } from '@iota/core';
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

const MAX_UNLOCK_ATTEMPTS = 3;
const WALLET_LOCK_DURATION_IN_MS = 60000;
const WALLET_LOCK_STORAGE_KEY = 'wallet_extension_lock_time';

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
    const [invalidPasswordAttempts, setInvalidPasswordAttempts] = useState<number>(0);
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

    function getRemainingLockTime() {
        const storedLockTime = localStorage.getItem(WALLET_LOCK_STORAGE_KEY);
        if (!storedLockTime) return 0;
        const elapsedTime = Math.floor((Date.now() - parseInt(storedLockTime)) / 1000);
        return elapsedTime < WALLET_LOCK_DURATION_IN_MS / 1000
            ? WALLET_LOCK_DURATION_IN_MS / 1000 - elapsedTime
            : 0;
    }

    function startLockTimer(duration: number) {
        setIsLockedOut(true);
        setRemainingLockTime(duration);
        localStorage.setItem(WALLET_LOCK_STORAGE_KEY, Date.now().toString());
    }

    useEffect(() => {
        const remainingTime = getRemainingLockTime();
        if (remainingTime > 0) startLockTimer(remainingTime * 1000);
    }, []);

    useEffect(() => {
        if (!isLockedOut || remainingLockTime <= 0) return;
        const timer = setInterval(() => {
            setRemainingLockTime((prev) => {
                if (prev <= 1000) {
                    setIsLockedOut(false);
                    localStorage.removeItem(WALLET_LOCK_STORAGE_KEY);
                    clearInterval(timer);
                    return 0;
                }
                return prev - 1000;
            });
        }, 1000);
        return () => clearInterval(timer);
    }, [isLockedOut, remainingLockTime]);

    async function handleOnSubmit({ password }: { password: string }) {
        if (isLockedOut) return;
        try {
            if (verify) {
                await backgroundService.verifyPassword({ password });
            }
            await onSubmit(password);
            reset();
            setInvalidPasswordAttempts(0);
        } catch (e) {
            const newAttempts = invalidPasswordAttempts + 1;
            setInvalidPasswordAttempts(newAttempts);
            if (newAttempts >= MAX_UNLOCK_ATTEMPTS) {
                startLockTimer(WALLET_LOCK_DURATION_IN_MS);
                toast.error('Too many attempts, please try again later');
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
                                    errorMessage={
                                        isLockedOut
                                            ? `You can try again in ${remainingLockTime} seconds`
                                            : form.formState.errors.password?.message
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
                                        disabled={isSubmitting || !isValid || isLockedOut}
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
