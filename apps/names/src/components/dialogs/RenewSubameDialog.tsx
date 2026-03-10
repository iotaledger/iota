// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Info, Warning } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonType,
    Dialog,
    DialogBody,
    DialogContent,
    DialogPosition,
    DisplayStats,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    Panel,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { isSubname, NameRecord, normalizeIotaName } from '@iota/iota-names-sdk';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import toast from 'react-hot-toast';

import { NameRecordData, queryKey, useNameRecord, useRegistrationNfts } from '@/hooks';
import { useNamesConfig } from '@/hooks/useNamesConfig';
import { NameUpdate, useUpdateNameTransaction } from '@/hooks/useUpdateNameTransaction';
import { RegistrationNft } from '@/lib/interfaces';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { ampli } from '@/lib/utils/analytics/ampli';
import { formatExpirationDate } from '@/lib/utils/format/formatExpirationDate';
import {
    getNameObject,
    getNamePermissions,
    getParentObject,
    isGracePeriodExpired,
} from '@/lib/utils/names';

function createRenewUpdates({
    nameRecord,
    ownedNames = [],
    ownedSubnames = [],
}: {
    nameRecord?: NameRecord;
    ownedNames?: RegistrationNft[];
    ownedSubnames?: RegistrationNft[];
}) {
    const isNameSubname = nameRecord?.name ? isSubname(nameRecord.name) : false;
    const namePermissions = nameRecord ? getNamePermissions(nameRecord) : null;
    const isExpired = nameRecord ? isGracePeriodExpired(nameRecord) : false;

    const updates: NameUpdate[] = [];

    if (isNameSubname && nameRecord && namePermissions?.allowTimeExtension && !isExpired) {
        const objectId = getNameObject(ownedSubnames, nameRecord.name);
        const parentObject = getParentObject(ownedNames, ownedSubnames, nameRecord.name);
        if (objectId && parentObject) {
            // Only allow extending the expiration time if its less than its parent
            const expiresBeforeParent = nameRecord.expirationDate < parentObject?.expirationDate;
            if (expiresBeforeParent) {
                updates.push({
                    type: 'renew-subname',
                    nftId: objectId,
                    expirationDate: parentObject.expirationDate,
                });
            }
        }
    }
    return updates;
}

interface RenewDialogProps {
    name: string;
    setOpen: (bool: boolean) => void;
    onRenew?: () => void;
}

export function RenewSubnameDialog({ setOpen, name, onRenew }: RenewDialogProps) {
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const account = useCurrentAccount();
    const { data: nameRecordData, isLoading: isLoadingNameRecord } = useNameRecord(name);
    const { data: config, isLoading: isLoadingConfig } = useNamesConfig();

    // We are sure that only owned names are passed here
    const nameRecord = nameRecordData as
        | Extract<NameRecordData, { type: 'unavailable' }>
        | undefined;

    const { data: ownedNames } = useRegistrationNfts('name');
    const { data: ownedSubnames } = useRegistrationNfts('subname');

    const updates = createRenewUpdates({
        nameRecord: nameRecord?.nameRecord,
        ownedNames,
        ownedSubnames,
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

    const { mutateAsync: handleConfirmRenewName, isPending: isSigning } = useMutation({
        async mutationFn() {
            if (!updateNameTransaction) return;
            const transactionResult = await signAndExecuteTransaction({
                transaction: updateNameTransaction.transaction,
            });

            await iotaClient.waitForTransaction({
                digest: transactionResult.digest,
            });
        },
        onSuccess() {
            setOpen(false);
            if (onRenew) {
                onRenew();
            }
            queryClient.invalidateQueries({
                queryKey: queryKey.nameRecord(name),
            });
            queryClient.invalidateQueries({
                queryKey: queryKey.getObject(name),
            });
            const expirationTime = expirationDate ? expirationDate.getTime() : Date.now();

            ampli.renewedSubname({
                name,
                // TODO expirationType `custom` will be added after implement https://github.com/iotaledger/iota-names/issues/915
                expirationType: 'parent',
                expirationTime,
            });
            toast.success('Subname renewed successfully');
        },
        onError(error) {
            toast.error(getUserFriendlyErrorMessage(error));
        },
    });

    function handleCancelRenewName() {
        setOpen(false);
    }

    const canRenew = nameRecord && updates.length > 0;
    const expirationDate = (() => {
        if (nameRecord?.nameRecord && ownedNames && ownedSubnames) {
            const expirationTime = getParentObject(
                ownedNames,
                ownedSubnames,
                nameRecord.nameRecord.name,
            )?.expirationDate;
            if (expirationTime) {
                return expirationTime;
            }
        }
    })();

    const renewalTime =
        expirationDate && nameRecord && nameRecord.nameRecord
            ? expirationDate.getTime() - nameRecord.nameRecord.expirationDate.getTime()
            : 0;

    const isBelowMinimumRenewalPeriod =
        renewalTime >= 0 && config?.subnamesConfig
            ? renewalTime < config.subnamesConfig.minimum_duration
            : false;

    const currentExpirationDate = nameRecord?.nameRecord
        ? formatExpirationDate(nameRecord.nameRecord.expirationDate)
        : null;
    const nextExpirationDate = expirationDate ? formatExpirationDate(expirationDate) : null;

    const isLoadingData = isLoadingNameRecord || isLoadingConfig;
    const isLoading =
        isLoadingUpdateNameTransaction || isSendingTransaction || isSigning || isLoadingData;

    const disableSave =
        isLoading || !canRenew || !updateNameTransaction || isBelowMinimumRenewalPeriod;
    const cleanName = normalizeIotaName(nameRecord?.nameRecord?.name || name);

    return (
        <Dialog open onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header title="Renew Subname" onClose={() => setOpen(false)} />
                <DialogBody>
                    <div className="flex flex-col justify-between h-full items-center">
                        <div className="flex flex-col w-full gap-y-md">
                            <Panel bgColor="bg-names-neutral-12">
                                <div className="px-md py-lg">
                                    <span className="text-names-neutral-100 text-headline-sm break-words">
                                        {cleanName}
                                    </span>
                                </div>
                            </Panel>
                            {isBelowMinimumRenewalPeriod && (
                                <InfoBox
                                    type={InfoBoxType.Warning}
                                    icon={<Info />}
                                    title="This subname already has the same expiration as its parent."
                                    style={InfoBoxStyle.Default}
                                    supportingText={`Please extend the parent expiration first.`}
                                />
                            )}
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
                            <div className="flex flex-row gap-x-sm w-full">
                                <DisplayStats
                                    label="Current Registration Expires"
                                    value={currentExpirationDate}
                                />
                                {canRenew && (
                                    <DisplayStats
                                        label="Next Expiration Date"
                                        value={nextExpirationDate}
                                    />
                                )}
                            </div>
                            <div className="flex w-full flex-row gap-x-xs">
                                <Button
                                    type={ButtonType.Secondary}
                                    text="Cancel"
                                    onClick={handleCancelRenewName}
                                    fullWidth
                                />
                                <Button
                                    icon={isLoading ? <LoadingIndicator /> : null}
                                    type={ButtonType.Primary}
                                    text="Renew"
                                    onClick={() => handleConfirmRenewName()}
                                    disabled={disableSave}
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
