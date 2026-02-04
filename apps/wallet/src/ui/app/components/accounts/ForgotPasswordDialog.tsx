// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { toast } from '@iota/core';
import {
    Button,
    ButtonHtmlType,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    Header,
} from '@iota/apps-ui-kit';
import { useLogoutMutation } from '_hooks';
import { Warning } from '@iota/apps-ui-icons';

interface ForgotPasswordDialogProps {
    isOpen: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function ForgotPasswordDialog({ isOpen, setOpen }: ForgotPasswordDialogProps) {
    const logoutMutation = useLogoutMutation();

    function onClose() {
        setOpen(false);
    }

    async function resetWallet() {
        try {
            await logoutMutation.mutateAsync(undefined, {
                onSuccess: () => {
                    window.location.reload();
                },
            });
        } catch (e) {
            toast.error((e as Error).message || 'Failed to reset wallet. Please try again.');
        }
    }

    return (
        <Dialog open={isOpen} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="" onClose={onClose} />
                <DialogBody>
                    <div className="flex flex-col gap-y-md">
                        <div className="flex flex-col gap-y-sm">
                            <div className="self-center rounded-full bg-iota-neutral-96 p-md dark:bg-iota-neutral-10">
                                <Warning className="h-12 w-12 text-iota-neutral-60 dark:text-iota-neutral-40" />
                            </div>
                            <div className="flex flex-col gap-y-xs py-xs text-center">
                                <span className="text-headline-sm text-iota-neutral-10 dark:text-iota-neutral-92">
                                    Forgot password?
                                </span>
                                <p className="text-body-md text-iota-neutral-40 dark:text-iota-neutral-60">
                                    Resetting your password requires resetting your wallet. <br />
                                    This will permanently delete all wallet data from this device,
                                    and you’ll need to set up your accounts again.
                                    <br />
                                    IOTA Wallet cannot recover your password.
                                </p>
                            </div>
                        </div>
                        <Button
                            htmlType={ButtonHtmlType.Submit}
                            type={ButtonType.Destructive}
                            text="Reset Wallet"
                            fullWidth
                            onClick={resetWallet}
                        />
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
