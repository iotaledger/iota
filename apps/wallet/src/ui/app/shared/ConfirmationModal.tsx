// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { Button, ButtonType, Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';

export interface ConfirmationModalProps {
    isOpen: boolean;
    title?: string;
    hint?: string;
    confirmText?: string;
    cancelText?: string;
    onResponse: (confirmed: boolean) => Promise<void>;
}

export function ConfirmationModal({
    isOpen,
    title = 'Are you sure?',
    hint,
    confirmText = 'Confirm',
    cancelText = 'Cancel',
    onResponse,
}: ConfirmationModalProps) {
    const [isConfirmLoading, setIsConfirmLoading] = useState(false);
    const [isCancelLoading, setIsCancelLoading] = useState(false);
    return (
        <Dialog
            open={isOpen}
            onOpenChange={async (open) => {
                if (open || isCancelLoading || isConfirmLoading) {
                    return;
                }
                setIsCancelLoading(true);
                await onResponse(false);
                setIsCancelLoading(false);
            }}
        >
            <DialogContent containerId="overlay-portal-container">
                <Header title={title} />
                <DialogBody>
                    <div className="flex flex-col gap-lg">
                        {hint ? <div className="text-body-md">{hint}</div> : null}
                        <div className="flex gap-xs">
                            <Button
                                type={ButtonType.Secondary}
                                text={cancelText}
                                disabled={isConfirmLoading}
                                onClick={async () => {
                                    setIsCancelLoading(true);
                                    await onResponse(false);
                                    setIsCancelLoading(false);
                                }}
                                fullWidth
                            />
                            <Button
                                type={ButtonType.Primary}
                                text={confirmText}
                                disabled={isCancelLoading}
                                onClick={async () => {
                                    setIsConfirmLoading(true);
                                    await onResponse(true);
                                    setIsConfirmLoading(false);
                                }}
                                fullWidth
                            />
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
