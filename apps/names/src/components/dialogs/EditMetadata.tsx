// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

'use client';

import { Warning } from '@iota/apps-ui-icons';
import {
    Button,
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
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { ALLOWED_METADATA, isSubname, normalizeIotaName } from '@iota/iota-names-sdk';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useState } from 'react';
import toast from 'react-hot-toast';

import {
    NameRecordData,
    NameUpdate,
    queryKey,
    useNameRecord,
    useRegistrationNfts,
    useUpdateNameTransaction,
} from '@/hooks';
import { METADATA_KEYS, SCHEMAS } from '@/lib/schemas';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { getNameObject } from '@/lib/utils/names';

interface EditMetadataDialogProps {
    name: string;
    setOpen: (bool: boolean) => void;
}

const METADATA_FIELDS = METADATA_KEYS.map(({ label, allowedKey }) => ({
    key: ALLOWED_METADATA[allowedKey],
    label,
}));

export function EditMetadataDialog({ name, setOpen }: EditMetadataDialogProps) {
    const account = useCurrentAccount();
    const iotaClient = useIotaClient();
    const queryClient = useQueryClient();

    const { data: nameRecordData } = useNameRecord(name);
    const { data: subnamesOwned } = useRegistrationNfts('subname');

    const nameRecord = nameRecordData as
        | Extract<NameRecordData, { type: 'unavailable' }>
        | undefined;

    const [validationErrors, setValidationErrors] = useState<Record<string, string>>({});
    const [metadata, setMetadata] = useState(() => {
        const initial: Record<string, { selected: boolean; data: string }> = {};
        METADATA_FIELDS.forEach(({ key }) => {
            const data = nameRecord?.nameRecord.data[key];
            initial[key] = {
                selected: data !== undefined,
                data: data || '',
            };
        });
        return initial;
    });

    const updates: NameUpdate[] = (() => {
        const updates: NameUpdate[] = [];
        const isNameSubname = nameRecord?.nameRecord
            ? isSubname(nameRecord.nameRecord.name)
            : false;
        const nftId =
            isNameSubname && nameRecord
                ? getNameObject(subnamesOwned ?? [], nameRecord.nameRecord.name)
                : nameRecord?.nameRecord.nftId;

        if (nameRecord && nftId) {
            METADATA_FIELDS.forEach(({ key }) => {
                const field = metadata[key];
                const currentField = nameRecord.nameRecord.data[key];

                if (field.selected && (currentField === undefined || field.data !== currentField)) {
                    updates.push({
                        type: 'set-data',
                        nftId,
                        key: key,
                        value: field.data,
                        isSubname: isNameSubname,
                    });
                } else if (!field.selected && currentField !== undefined) {
                    updates.push({
                        type: 'unset-data',
                        nftId,
                        key: key,
                        isSubname: isNameSubname,
                    });
                }
            });
        }

        return updates;
    })();

    const {
        data: updateNameTransaction,
        isLoading: isUpdateNameLoading,
        error: updateNameError,
    } = useUpdateNameTransaction({
        address: account?.address || '',
        updates,
    });

    const { mutateAsync: signAndExecuteTransaction, isPending: isSigning } =
        useSignAndExecuteTransaction();

    const { mutate: handleApply, isPending: isSaving } = useMutation({
        async mutationFn() {
            if (!updateNameTransaction) return;
            const tx = await signAndExecuteTransaction({
                transaction: updateNameTransaction.transaction,
            });
            await iotaClient.waitForTransaction({ digest: tx.digest });
        },
        onSuccess() {
            setOpen(false);
            toast.success(`Successfully updated metadata for ${normalizeIotaName(name)}`);
            queryClient.invalidateQueries({ queryKey: queryKey.nameRecord(name) });
        },
        onError: (error) => {
            toast.error(getUserFriendlyErrorMessage(error));
        },
    });

    function validateField(key: string, value: string): string | null {
        const def = METADATA_KEYS.find((item) => ALLOWED_METADATA[item.allowedKey] === key);
        if (!def) return null;
        const schema = SCHEMAS[def.allowedKey];
        if (!schema) return null;
        const parsed = schema.safeParse(value);
        if (parsed.success) return null;
        return parsed.error.issues[0]?.message || 'Invalid value';
    }

    function hasSelectedEmptyFields(): boolean {
        return METADATA_FIELDS.some(({ key }) => {
            const field = metadata[key];
            return field?.selected && (!field.data || field.data.trim() === '');
        });
    }

    function updateMetadataData(key: string, data: string) {
        setMetadata((prev) => ({
            ...prev,
            [key]: { ...prev[key], data },
        }));

        if (data && data.trim() !== '') {
            const error = validateField(key, data);
            setValidationErrors((prev) => ({
                ...prev,
                [key]: error || '',
            }));
        } else {
            const field = metadata[key];
            if (field?.selected) {
                setValidationErrors((prev) => ({
                    ...prev,
                    [key]: 'Field is required',
                }));
            } else {
                setValidationErrors((prev) => {
                    const newErrors = { ...prev };
                    delete newErrors[key];
                    return newErrors;
                });
            }
        }
    }

    function toggleMetadata(key: string) {
        setMetadata((prev) => ({
            ...prev,
            [key]: {
                ...prev[key],
                selected: !metadata[key]?.selected,
                data: !metadata[key]?.selected ? prev[key]?.data || '' : '',
            },
        }));
        if (metadata[key]?.selected) {
            setValidationErrors((prev) => {
                const newErrors = { ...prev };
                delete newErrors[key];
                return newErrors;
            });
        }
    }

    const hasEmptyRequiredFields = hasSelectedEmptyFields();
    const hasValidationErrors = Object.values(validationErrors).some((error) => error);
    const isLoading = isUpdateNameLoading || isSigning || isSaving;
    const disableApply =
        isLoading ||
        updates.length === 0 ||
        !updateNameTransaction ||
        hasValidationErrors ||
        hasEmptyRequiredFields;

    return (
        <Dialog open onOpenChange={setOpen}>
            <DialogContent isFixedPosition position={DialogPosition.Right}>
                <Header title="Edit Metadata" onClose={() => setOpen(false)} />
                <DialogBody>
                    <div className="flex flex-col h-full justify-between">
                        <div className="flex flex-col gap-md">
                            <p className="text-label-md y color text-names-neutral-70">
                                Select type
                            </p>
                            <div className="flex flex-wrap gap-xs">
                                {METADATA_FIELDS.map(({ key, label }) => (
                                    <Chip
                                        key={key}
                                        label={label}
                                        selected={metadata[key]?.selected || false}
                                        type={
                                            metadata[key]?.selected
                                                ? ChipType.Brand
                                                : ChipType.Outline
                                        }
                                        onClick={() => toggleMetadata(key)}
                                        aria-label={`${metadata[key]?.selected ? 'Deselect' : 'Select'} ${label} field`}
                                    />
                                ))}
                            </div>
                            <div className="flex flex-col gap-xs">
                                {METADATA_FIELDS.map(
                                    ({ key, label }) =>
                                        metadata[key]?.selected && (
                                            <Input
                                                key={key}
                                                type={InputType.Text}
                                                label={label}
                                                value={metadata[key]?.data || ''}
                                                onChange={({ target: { value } }) =>
                                                    updateMetadataData(key, value)
                                                }
                                                errorMessage={validationErrors[key] || undefined}
                                                data-amp-mask
                                            />
                                        ),
                                )}
                            </div>
                        </div>
                    </div>
                </DialogBody>
                <div className="flex flex-col gap-y-md gap-2 p-md--rs">
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
                            onClick={() => setOpen(false)}
                            fullWidth
                        />
                        <Button
                            icon={isLoading ? <LoadingIndicator /> : null}
                            text="Apply"
                            disabled={disableApply}
                            type={ButtonType.Primary}
                            onClick={() => handleApply()}
                            fullWidth
                        />
                    </div>
                </div>
            </DialogContent>
        </Dialog>
    );
}
