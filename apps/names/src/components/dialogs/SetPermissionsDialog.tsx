// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Info, Warning } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonType,
    Checkbox,
    Dialog,
    DialogBody,
    DialogContent,
    DialogPosition,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ChangeEvent, useEffect, useState } from 'react';
import toast from 'react-hot-toast';

import { NameRecordData, queryKey, useNameRecord, useRegistrationNfts } from '@/hooks';
import { NameUpdate, useUpdateNameTransaction } from '@/hooks/useUpdateNameTransaction';
import { RegistrationNft } from '@/lib/interfaces';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { getNamePermissions, getParentObject, isNameRecordExpired } from '@/lib/utils/names';

function createSetUpdates({
    name,
    ownedNames,
    ownedSubnames,
    editIsAllowingRenew,
    editIsAllowSubnames,
    namePermissions,
}: {
    name: string;
    ownedNames: RegistrationNft[];
    ownedSubnames: RegistrationNft[];
    editIsAllowingRenew: boolean;
    editIsAllowSubnames: boolean;
    namePermissions?: ReturnType<typeof getNamePermissions>;
}) {
    const updates: NameUpdate[] = [];
    const nftId = getParentObject(ownedNames ?? [], ownedSubnames ?? [], name)?.id;
    if (
        nftId &&
        (editIsAllowingRenew !== namePermissions?.allowTimeExtension ||
            editIsAllowSubnames !== namePermissions?.allowChildCreation)
    ) {
        updates.push({
            type: 'edit-setup',
            name,
            parentNftId: nftId,
            allowChildCreation: editIsAllowSubnames,
            allowTimeExtension: editIsAllowingRenew,
        });
    }

    return {
        updates,
    };
}

type CreateSubnameProps = {
    name: string;
    setOpen: (bool: boolean) => void;
};

export function SetPermissionsDialog({ name, setOpen }: CreateSubnameProps) {
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const account = useCurrentAccount();
    const { data: ownedNames = [] } = useRegistrationNfts('name');
    const { data: ownedSubnames = [] } = useRegistrationNfts('subname');
    const { data: nameRecordData, isLoading: isNameRecordLoading } = useNameRecord(name);

    // We are sure that only owned names are passed here
    const nameRecord = nameRecordData as
        | Extract<NameRecordData, { type: 'unavailable' }>
        | undefined;

    const namePermissions = nameRecord?.nameRecord
        ? getNamePermissions(nameRecord.nameRecord)
        : undefined;

    const isExpired = nameRecord?.nameRecord ? isNameRecordExpired(nameRecord?.nameRecord) : false;

    // Editable values
    const [editIsAllowingRenew, setEditIsAllowingRenew] = useState<boolean>(false);
    const [editIsAllowSubnames, setEditIsAllowSubnames] = useState<boolean>(false);

    // Sync permissions
    useEffect(() => {
        if (namePermissions) {
            setEditIsAllowingRenew(namePermissions.allowTimeExtension);
            setEditIsAllowSubnames(namePermissions.allowChildCreation);
        }
    }, [namePermissions?.allowChildCreation, namePermissions?.allowTimeExtension]);

    const { updates } = createSetUpdates({
        name,
        ownedNames,
        ownedSubnames,
        editIsAllowingRenew,
        editIsAllowSubnames,
        namePermissions,
    });

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

    const { mutate: save, isPending: isSaving } = useMutation({
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
            const editSetupUpdate = updates.find((u) => u.type === 'edit-setup');
            if (editSetupUpdate) {
                queryClient.invalidateQueries({
                    queryKey: queryKey.nameRecord(editSetupUpdate.name),
                });
            }
            toast.success('Permissions updated successfully');
            closeDialog();
        },
        onError: (error: Error) => {
            toast.error(getUserFriendlyErrorMessage(error));
        },
    });

    function closeDialog() {
        setOpen(false);
    }

    const handleAllowRenewChange = ({ target: { checked } }: ChangeEvent<HTMLInputElement>) => {
        setEditIsAllowingRenew(checked);
    };

    const handleAllowSubnameChange = ({ target: { checked } }: ChangeEvent<HTMLInputElement>) => {
        setEditIsAllowSubnames(checked);
    };

    const isLoading = isSaving || isLoadingUpdateNameTransaction || isSendingTransaction;

    const disableEdit = isNameRecordLoading || isSendingTransaction || isExpired;
    const disableSave = updates.length === 0 || isLoading || isExpired || !updateNameTransaction;

    return (
        <Dialog open onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header title="Set permissions" onClose={closeDialog} />
                <DialogBody>
                    <div className="flex flex-col h-full justify-between">
                        <div className="flex flex-col h-full items-center gap-y-lg">
                            <InfoBox
                                type={InfoBoxType.Default}
                                style={InfoBoxStyle.Elevated}
                                icon={<Info />}
                                supportingText="Modify the permissions of your subname to allow subnames or renewals."
                            />
                            <div className="flex flex-col gap-y-md w-full">
                                <span className="text-label-lg text-names-neutral-92">
                                    Permissions
                                </span>
                                <Checkbox
                                    name="allow_subnames"
                                    isChecked={editIsAllowSubnames}
                                    isDisabled={disableEdit}
                                    onCheckedChange={handleAllowSubnameChange}
                                    label="Allow Subname to create additional Subnames"
                                />
                                <Checkbox
                                    name="allow_renew"
                                    isChecked={editIsAllowingRenew}
                                    isDisabled={disableEdit}
                                    onCheckedChange={handleAllowRenewChange}
                                    label="Allow Subname to renew expiration"
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
                                    text="Save"
                                    disabled={disableSave}
                                    onClick={() => save()}
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
