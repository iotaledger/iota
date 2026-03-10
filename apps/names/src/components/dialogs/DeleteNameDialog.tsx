// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Warning } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    DialogPosition,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    Panel,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { isSubname, normalizeIotaName } from '@iota/iota-names-sdk';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';

import { useRegistrationNfts } from '@/hooks';
import { queryKey } from '@/hooks/queryKey';
import { NameUpdate, useUpdateNameTransaction } from '@/hooks/useUpdateNameTransaction';
import { RegistrationNft } from '@/lib/interfaces/registration.interfaces';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { getNameObject } from '@/lib/utils/names';

type DeleteNameDialogProps = {
    nft: RegistrationNft;
    setOpen: (bool: boolean) => void;
};

export function DeleteNameDialog({ nft, setOpen }: DeleteNameDialogProps) {
    const iotaClient = useIotaClient();
    const queryClient = useQueryClient();
    const account = useCurrentAccount();

    const { data: subnamesOwned } = useRegistrationNfts('subname');

    const [deleteSeconds, setDeleteSeconds] = useState(5);

    const isNameSubname = nft ? isSubname(nft.name) : null;

    // Create updates
    const updates: NameUpdate[] = [];

    if (nft.isExpired) {
        const nftId = isNameSubname ? getNameObject(subnamesOwned ?? [], nft.name) : nft.id;
        if (nftId) {
            updates.push({
                type: 'delete-name',
                nft: nftId,
                isSubname: isNameSubname || false,
            });
        }
    }

    const {
        data: updateNameTransaction,
        error: updateNameError,
        isLoading: isLoadingUpdateNameTransaction,
    } = useUpdateNameTransaction({
        address: account?.address || '',
        updates,
    });

    const { mutateAsync: signAndExecuteTransaction, isPending: isSendingTransaction } =
        useSignAndExecuteTransaction();

    const { mutate: deleteName, isPending: isSaving } = useMutation({
        async mutationFn() {
            if (!updateNameTransaction) return;
            const transaction = await signAndExecuteTransaction({
                transaction: updateNameTransaction.transaction,
            });
            await iotaClient.waitForTransaction({
                digest: transaction.digest,
            });
        },
        onSuccess: () => {
            queryClient.invalidateQueries({
                queryKey: queryKey.ownedObjects(account?.address || ''),
            });
            toast.success(
                `Successfully deleted expired name ${normalizeIotaName(nft.name, 'at', { truncateLongParts: true })}`,
            );
            closeDialog();
        },
        onError: (error) => {
            toast.error(getUserFriendlyErrorMessage(error));
        },
    });

    function closeDialog() {
        setOpen(false);
    }

    const isLoading = isLoadingUpdateNameTransaction || isSaving || isSendingTransaction;
    const disableDeleteButton =
        isLoadingUpdateNameTransaction || isLoading || !nft.isExpired || !updateNameTransaction;
    const deleteActionNotAllowed = deleteSeconds > 0;
    useEffect(() => {
        if (!deleteActionNotAllowed) return;

        const id = setInterval(() => {
            setDeleteSeconds((s) => s - 1);
        }, 1_000);

        return () => clearInterval(id);
    }, [deleteActionNotAllowed]);

    return (
        <Dialog open onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header title="Delete" onClose={closeDialog} />
                <DialogBody>
                    <div className="flex flex-col justify-between h-full items-center">
                        <div className="flex flex-col w-full gap-y-md">
                            <div className="flex flex-col items-start gap-y-md">
                                <p className="text-title-md text-names-neutral-100">
                                    Are you sure you want to delete this name?
                                </p>
                                <Panel bgColor="bg-names-neutral-12">
                                    <div className="p-md">
                                        <span className="text-names-neutral-100 text-headline-sm">
                                            {normalizeIotaName(nft.name)}
                                        </span>
                                    </div>
                                </Panel>
                                <InfoBox
                                    type={InfoBoxType.Warning}
                                    style={InfoBoxStyle.Default}
                                    icon={<Warning />}
                                    title="Permanently remove name"
                                    supportingText="This action is irreversible and will permanently remove your ownership of the name"
                                />
                            </div>
                        </div>
                        <div className="flex flex-col w-full gap-y-md">
                            {updateNameError ? (
                                <InfoBox
                                    type={InfoBoxType.Error}
                                    style={InfoBoxStyle.Elevated}
                                    icon={<Warning />}
                                    title="Error"
                                    supportingText={getUserFriendlyErrorMessage(updateNameError)}
                                />
                            ) : null}
                            <div className="flex w-full flex-row gap-x-xs">
                                <Button
                                    type={ButtonType.Secondary}
                                    text="Cancel"
                                    onClick={closeDialog}
                                    fullWidth
                                />
                                <Button
                                    icon={isLoading ? <LoadingIndicator /> : null}
                                    text={
                                        deleteActionNotAllowed
                                            ? `Delete in ${deleteSeconds}s`
                                            : 'Delete'
                                    }
                                    disabled={disableDeleteButton || deleteActionNotAllowed}
                                    type={ButtonType.Destructive}
                                    onClick={() => deleteName()}
                                    fullWidth
                                />
                            </div>
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}
