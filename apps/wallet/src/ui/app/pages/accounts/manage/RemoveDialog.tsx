// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAccounts, useBackgroundClient } from '_hooks';
import { useMutation } from '@tanstack/react-query';
import {
    Button,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
} from '@iota/apps-ui-kit';
import { toast } from '@iota/core';
import { Warning } from '@iota/apps-ui-icons';
import { VerifyPasswordModal } from '_src/ui/app/components';
import { useState } from 'react';
import { ampli, ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE } from '_src/shared/analytics';

interface RemoveDialogProps {
    accountID: string;
    isOpen: boolean;
    setOpen: (isOpen: boolean) => void;
}

export function RemoveDialog({ isOpen, setOpen, accountID }: RemoveDialogProps) {
    const allAccounts = useAccounts();
    const backgroundClient = useBackgroundClient();
    const removeAccountMutation = useMutation({
        mutationKey: ['remove account mutation', accountID],
        mutationFn: async () => {
            // Get account type before deletion for analytics
            const account = allAccounts?.data?.find((acc) => acc.id === accountID);
            const accountType = account?.type;

            await backgroundClient.removeAccount({ accountID: accountID });

            // Track account deletion event
            if (accountType) {
                ampli.deletedAccount({
                    accountType: ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE[accountType],
                });
            }

            setOpen(false);
        },
    });
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(true);

    const totalAccounts = allAccounts?.data?.length || 0;

    function handleCancel() {
        setPasswordModalVisible(true);
        setOpen(false);
    }

    function handleRemove() {
        removeAccountMutation.mutate(undefined, {
            onSuccess: () => toast.success('Account removed'),
            onError: (e) => toast.error((e as Error)?.message || 'Something went wrong'),
        });
    }

    if (isPasswordModalVisible) {
        return (
            <VerifyPasswordModal
                open={isOpen}
                onVerify={() => {
                    setPasswordModalVisible(false);
                }}
                onClose={handleCancel}
            />
        );
    }

    return (
        <Dialog open={isOpen} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Remove account" onClose={handleCancel} />
                <DialogBody>
                    <div className="flex flex-col gap-y-md">
                        <div className="text-body-md">
                            Are you sure you want to remove this account?
                        </div>
                        {totalAccounts === 1 ? (
                            <InfoBox
                                type={InfoBoxType.Warning}
                                supportingText="Removing this account will require you to set up your IOTA wallet again."
                                icon={<Warning />}
                                style={InfoBoxStyle.Elevated}
                            />
                        ) : null}
                        <div className="flex gap-xs">
                            <Button
                                fullWidth
                                type={ButtonType.Secondary}
                                text="Cancel"
                                onClick={handleCancel}
                            />
                            <Button
                                fullWidth
                                type={ButtonType.Destructive}
                                text="Remove"
                                onClick={handleRemove}
                            />
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
