// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Add, CheckmarkFilled, Copy, Link, Warning } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonSize,
    ButtonType,
    Chip,
    ChipType,
    Dialog,
    DialogBody,
    DialogContent,
    DialogPosition,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    Input,
    InputType,
    LoadingIndicator,
    Panel,
    Toggle,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { isSubname, normalizeIotaName } from '@iota/iota-names-sdk';
import { formatAddress, isValidIotaAddress } from '@iota/iota-sdk/utils';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import clsx from 'clsx';
import Image from 'next/image';
import { ChangeEvent, useEffect, useState } from 'react';
import toast from 'react-hot-toast';

import { NameRecordData, queryKey, useNameRecord, useRegistrationNfts } from '@/hooks';
import { useGetPublicName } from '@/hooks/useGetPublicName';
import { NameUpdate, useUpdateNameTransaction } from '@/hooks/useUpdateNameTransaction';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { ampli } from '@/lib/utils/analytics/ampli';
import { copyToClipboard } from '@/lib/utils/copyToClipboard';
import { getNameObject, isNameRecordExpired } from '@/lib/utils/names';

interface ConnectToAddressDialogProps {
    name: string;
    setOpen: (open: boolean) => void;
}

export function ConnectToAddressDialog({ name, setOpen }: ConnectToAddressDialogProps) {
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const account = useCurrentAccount();

    const [editTargetAddress, setEditTargetAddress] = useState<string>('');
    const [editIsPublicName, setEditIsPublicName] = useState<boolean>(false);

    const { data: nameRecordData, isLoading: isNameRecordLoading } = useNameRecord(name);
    const { data: ownedSubnames } = useRegistrationNfts('subname');
    const { data: addressName } = useGetPublicName(account?.address ?? '');

    const nameRecord = nameRecordData as
        | Extract<NameRecordData, { type: 'unavailable' }>
        | undefined;

    const isNameSubname = nameRecord?.nameRecord ? isSubname(nameRecord.nameRecord.name) : false;
    const isExpired = nameRecord?.nameRecord ? isNameRecordExpired(nameRecord.nameRecord) : false;
    const currentTargetAddress = nameRecord?.nameRecord.targetAddress ?? '';

    // Sync name target address
    useEffect(() => {
        setEditTargetAddress(currentTargetAddress);
    }, [currentTargetAddress]);

    // Sync address current public name
    useEffect(() => {
        setEditIsPublicName(addressName === name);
    }, [addressName]);

    const hasAddressChange = editTargetAddress !== currentTargetAddress;
    const hasPublicChanged = (addressName === name) !== editIsPublicName;
    const isValidAddressOrEmpty = editTargetAddress === '' || isValidIotaAddress(editTargetAddress);
    const hasChanges = (hasAddressChange && isValidAddressOrEmpty) || hasPublicChanged;
    const isTargetingCurrentAddress = editTargetAddress === account?.address;

    const updates: NameUpdate[] = [];
    const nftId = isNameSubname
        ? getNameObject(ownedSubnames ?? [], nameRecord?.nameRecord.name ?? '')
        : nameRecord?.nameRecord.nftId;

    if (hasPublicChanged && !editIsPublicName) {
        updates.push({ type: 'unset-public' });
    }

    if (hasAddressChange && nftId) {
        updates.push({
            type: 'set-target-address',
            nftId,
            address: editTargetAddress || undefined,
            isSubname: !!isNameSubname,
        });
    }

    if (hasPublicChanged && editIsPublicName) {
        if (isTargetingCurrentAddress) {
            updates.push({ type: 'set-public', name });
        } else {
            // Dont allow setting a name as public if the
            // edit address does not match the current address
            setEditIsPublicName(false);
        }
    }

    const {
        data: updateNameTransaction,
        error: updateNameError,
        isLoading: isLoadingTx,
    } = useUpdateNameTransaction({ address: account?.address || '', updates });

    const { mutateAsync: signAndExecuteTransaction, isPending: isSigning } =
        useSignAndExecuteTransaction();

    const [appliedUpdates, setAppliedUpdates] = useState<NameUpdate[]>([]);

    const {
        mutate: apply,
        isPending: isApplying,
        isSuccess,
    } = useMutation({
        async mutationFn() {
            if (!updateNameTransaction) return;
            const txResult = await signAndExecuteTransaction({
                transaction: updateNameTransaction.transaction,
            });
            await iotaClient.waitForTransaction({ digest: txResult.digest });
        },
        onSuccess: () => {
            setAppliedUpdates(updates);
            queryClient.invalidateQueries({ queryKey: queryKey.nameRecord(name) });
            queryClient.invalidateQueries({
                queryKey: queryKey.publicName(account?.address || ''),
            });
            queryClient.invalidateQueries({
                queryKey: ['iota-name', 'default-name', account?.address],
            });

            const hasAddressUpdate = updates.some((update) => update.type === 'set-target-address');
            const hasSetPublicUpdate = updates.some((update) => update.type === 'set-public');

            if (hasAddressUpdate || hasSetPublicUpdate) {
                const addressType = isTargetingCurrentAddress
                    ? 'current'
                    : editTargetAddress
                      ? 'external'
                      : 'empty';

                ampli.connectedAddress({
                    name: cleanName,
                    addressType: addressType,
                    setNameAsDisplayed: hasSetPublicUpdate,
                });
            }

            if (editTargetAddress.length === 0) {
                toast.success(`Successfully disconnected ${cleanName}`);
                setOpen(false);
            }
        },
        onError: (error) => {
            toast.error(getUserFriendlyErrorMessage(error));
        },
    });

    function handleClose() {
        setOpen(false);
    }

    function handleAddressChange(e: ChangeEvent<HTMLInputElement>) {
        setEditTargetAddress(e.target.value);
    }

    function handleUseCurrent() {
        if (account?.address) setEditTargetAddress(account.address);
    }

    const isLoading = isApplying || isSigning || isLoadingTx;
    const disableEdit = isNameRecordLoading || isExpired || isSigning;
    const disableApply =
        !hasChanges || !isValidAddressOrEmpty || isExpired || isLoading || !updateNameTransaction;
    const cleanName = normalizeIotaName(name, 'at', { truncateLongParts: true });

    const showAddressWarning = !!addressName && addressName !== name && editIsPublicName;
    const errorMessage =
        editTargetAddress && !isValidAddressOrEmpty ? 'Not a valid IOTA address' : undefined;
    return (
        <Dialog open onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header
                    title={isSuccess ? 'Success' : 'Connect to Address'}
                    onClose={handleClose}
                />

                <DialogBody>
                    <div className="flex flex-col h-full w-full justify-between">
                        <div className="flex flex-col h-full w-full gap-y-lg">
                            {isSuccess ? (
                                <UpdatesResult name={name} updates={appliedUpdates} />
                            ) : (
                                <>
                                    <div className="flex flex-col gap-y-xxs">
                                        <span className="text-title-md text-names-neutral-100">
                                            Link to Address
                                        </span>
                                        <div
                                            className={clsx(
                                                '[&>div>label]:break-words',
                                                isTargetingCurrentAddress && 'pb-6',
                                            )}
                                        >
                                            <Input
                                                type={InputType.Text}
                                                label={`Enter a target address to connect to ${cleanName}`}
                                                placeholder="Enter Address"
                                                value={editTargetAddress}
                                                onChange={handleAddressChange}
                                                onClearInput={() => setEditTargetAddress('')}
                                                disabled={disableEdit}
                                                errorMessage={errorMessage}
                                                data-amp-mask
                                            />
                                        </div>
                                        {!isTargetingCurrentAddress && (
                                            <div className="flex justify-start w-full">
                                                <Button
                                                    text="Use Current Address"
                                                    icon={<Add />}
                                                    onClick={handleUseCurrent}
                                                    disabled={disableEdit}
                                                    size={ButtonSize.Small}
                                                    type={ButtonType.Ghost}
                                                />
                                            </div>
                                        )}
                                        {nameRecord?.nameRecord.targetAddress &&
                                            !hasAddressChange &&
                                            editIsPublicName && (
                                                <div
                                                    data-amp-mask
                                                    className="flex w-full break-all"
                                                >
                                                    <InfoBox
                                                        type={InfoBoxType.Success}
                                                        style={InfoBoxStyle.Default}
                                                        icon={<Link />}
                                                        title="Name connected to"
                                                        supportingText={
                                                            nameRecord.nameRecord.targetAddress
                                                        }
                                                    />
                                                </div>
                                            )}
                                    </div>
                                    {isTargetingCurrentAddress && (
                                        <Panel bgColor="bg-names-neutral-10 state-layer relative">
                                            <div
                                                className={clsx(
                                                    'flex flex-col rounded-lg gap-y-xxxs',
                                                    !disableEdit && isTargetingCurrentAddress
                                                        ? 'cursor-pointer'
                                                        : 'cursor-not-allowed',
                                                )}
                                                onClick={() => {
                                                    if (!disableEdit && isTargetingCurrentAddress) {
                                                        setEditIsPublicName(!editIsPublicName);
                                                    }
                                                }}
                                                role="button"
                                            >
                                                <div className="flex flex-row items-center gap-x-md p-md">
                                                    <div className="flex flex-col gap-y-xxs">
                                                        <span className="text-title-md text-names-neutral-100">
                                                            Use as your public name
                                                        </span>
                                                        <span className="text-body-sm text-names-neutral-70">
                                                            Your IOTA name will be publicly visible
                                                            and searchable (e.g., in blockchain
                                                            explorers), allowing others to find you
                                                            using {cleanName} instead of your
                                                            address.
                                                        </span>
                                                    </div>
                                                    <Toggle
                                                        isToggled={editIsPublicName}
                                                        isDisabled={
                                                            disableEdit ||
                                                            !isTargetingCurrentAddress
                                                        }
                                                        onChange={(checked) => {
                                                            setEditIsPublicName(checked);
                                                        }}
                                                    />
                                                </div>
                                                <Image
                                                    src="/public-name-showcase.gif"
                                                    width={352}
                                                    height={117}
                                                    alt={cleanName}
                                                    className="w-full h-[117px] object-cover rounded-b-xl"
                                                />

                                                {showAddressWarning && (
                                                    <InfoBox
                                                        type={InfoBoxType.Warning}
                                                        style={InfoBoxStyle.Default}
                                                        icon={<Warning />}
                                                        title="Address has a linked name!"
                                                        supportingText="Continuing will override the previous address's name"
                                                    />
                                                )}
                                            </div>
                                        </Panel>
                                    )}
                                </>
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
                            <div className="flex w-full flex-row gap-x-xs">
                                <Button
                                    type={ButtonType.Secondary}
                                    text="Cancel"
                                    onClick={handleClose}
                                    fullWidth
                                />
                                {isSuccess ? (
                                    <Button
                                        type={ButtonType.Primary}
                                        text="Finish"
                                        onClick={handleClose}
                                        fullWidth
                                    />
                                ) : (
                                    <Button
                                        type={ButtonType.Primary}
                                        text="Apply"
                                        icon={isLoading ? <LoadingIndicator /> : null}
                                        onClick={() => apply()}
                                        disabled={disableApply}
                                        fullWidth
                                    />
                                )}
                            </div>
                        </div>
                    </div>
                </DialogBody>
            </DialogContent>
        </Dialog>
    );
}

function UpdatesResult({ name, updates }: { name: string; updates: NameUpdate[] }) {
    const account = useCurrentAccount();

    function copyAddressToClipboard() {
        copyToClipboard(targetAddress);
    }

    const cleanName = normalizeIotaName(name, 'at', {
        truncateLongParts: true,
    });

    const isNamePublic = updates.some((update) => update.type === 'set-public');
    const targetAddress =
        updates.find((update) => update.type === 'set-target-address')?.address ??
        account?.address ??
        '';
    return (
        <div className="flex flex-col gap-y-md">
            <div className="flex flex-col gap-y-sm">
                <div className="text-headline-sm text-iota-neutral-100">{cleanName}</div>
                <div data-amp-mask>
                    <Chip
                        leadingElement={<Link className="w-4 h-4" />}
                        label={formatAddress(targetAddress)}
                        trailingElement={<Copy className="w-4 h-4" />}
                        onClick={copyAddressToClipboard}
                        type={isNamePublic ? ChipType.Success : ChipType.Elevated}
                        aria-label="Copy address to clipboard"
                    />
                </div>
            </div>
            <Panel bgColor="bg-names-neutral-10">
                <div className="flex flex-col gap-y-2 p-md text-start">
                    {updates.map((update) => {
                        switch (update.type) {
                            case 'set-public': {
                                return (
                                    <div
                                        className="flex flex-row items-center gap-xs"
                                        key={update.type}
                                    >
                                        <CheckmarkFilled className="size-5 text-names-neutral-50" />
                                        <span className="text-body-md text-names-neutral-92">{`${cleanName} is now publicly visible.`}</span>
                                    </div>
                                );
                            }

                            case 'unset-public': {
                                return (
                                    <div
                                        className="flex flex-row items-center gap-xs"
                                        key={update.type}
                                    >
                                        <CheckmarkFilled className="size-5 text-names-neutral-50" />
                                        <span className="text-body-md text-names-neutral-92">{`${cleanName} is no longer publicly visible.`}</span>
                                    </div>
                                );
                            }

                            case 'set-target-address': {
                                return (
                                    <div
                                        className="flex flex-row items-center gap-xs"
                                        key={update.type}
                                    >
                                        <CheckmarkFilled className="size-5 text-names-neutral-50" />
                                        <span className="text-body-md text-names-neutral-92">
                                            {update.address
                                                ? 'Address linked successfully'
                                                : `${cleanName} is no longer linked to an address`}
                                        </span>
                                    </div>
                                );
                            }

                            default:
                                return null;
                        }
                    })}
                </div>
            </Panel>
        </div>
    );
}
