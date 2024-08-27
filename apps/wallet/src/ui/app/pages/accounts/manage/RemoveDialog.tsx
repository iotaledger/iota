// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useAccounts } from '_app/hooks/useAccounts';
import { useBackgroundClient } from '_app/hooks/useBackgroundClient';
import { useMutation } from '@tanstack/react-query';
import { Button, ButtonType, Dialog, DialogBody, DialogContent, Header } from '@iota/apps-ui-kit';
import toast from 'react-hot-toast';

export function RemoveDialog({
    isOpen,
    setOpen,
    accountID,
}: {
    accountID: string;
    isOpen: boolean;
    setOpen: React.Dispatch<React.SetStateAction<boolean>>;
}) {
    const allAccounts = useAccounts();
    const totalAccounts = allAccounts?.data?.length || 0;
    const backgroundClient = useBackgroundClient();
    const removeAccountMutation = useMutation({
        mutationKey: ['remove account mutation', accountID],
        mutationFn: async () => {
            await backgroundClient.removeAccount({ accountID: accountID });
            setOpen(false);
        },
    });
    return (
        <Dialog open={isOpen} onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container">
                <Header title="Remove account" onClose={() => setOpen(false)} />
                <DialogBody>
                    <div className="mb-md text-body-md">
                        Are you sure you want to remove this account?
                    </div>
                    {totalAccounts === 1 ? (
                        <div className="text-center">
                            Removing this account will require you to set up your IOTA wallet again.
                        </div>
                    ) : null}
                    <div className="flex gap-xs">
                        <Button
                            fullWidth
                            type={ButtonType.Secondary}
                            text="Cancel"
                            onClick={() => setOpen(false)}
                        />
                        <Button
                            fullWidth
                            type={ButtonType.Primary}
                            text="Remove"
                            onClick={() => {
                                removeAccountMutation.mutate(undefined, {
                                    onSuccess: () => toast.success('Account removed'),
                                    onError: (e) =>
                                        toast.error(
                                            (e as Error)?.message || 'Something went wrong',
                                        ),
                                });
                            }}
                        />
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
