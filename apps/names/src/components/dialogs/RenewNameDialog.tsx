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
    DisplayStats,
    Header,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
    Panel,
    Select,
} from '@iota/apps-ui-kit';
import { useCurrentAccount, useIotaClient, useSignAndExecuteTransaction } from '@iota/dapp-kit';
import { NameRecord, normalizeIotaName } from '@iota/iota-names-sdk';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect, useState } from 'react';
import toast from 'react-hot-toast';

import { useIotaNamesClient } from '@/contexts';
import { NameRecordData, queryKey, useCalculatePrice, useNameRecord } from '@/hooks';
import { useNamesConfig } from '@/hooks/useNamesConfig';
import { NameUpdate, useUpdateNameTransaction } from '@/hooks/useUpdateNameTransaction';
import { getUserFriendlyErrorMessage } from '@/lib/utils';
import { ampli } from '@/lib/utils/analytics/ampli';
import { formatExpirationDate } from '@/lib/utils/format/formatExpirationDate';
import { getNamePermissions, getNameRenewableYears, isGracePeriodExpired } from '@/lib/utils/names';

import { CouponInputSelection } from '../CouponInputSelection';
import type { UserSetCoupon } from './PurchaseNameDialog';

function createRenewUpdates({
    nameRecord,
    renewYears,
    applyCoupons = false,
    coupon = '',
    address,
}: {
    nameRecord?: NameRecord;
    renewYears?: number;
    applyCoupons?: boolean;
    coupon?: string;
    address?: string;
}) {
    const namePermissions = nameRecord ? getNamePermissions(nameRecord) : null;
    const isExpired = nameRecord ? isGracePeriodExpired(nameRecord) : false;

    const updates: NameUpdate[] = [];

    // Renew names
    if (nameRecord && namePermissions?.allowTimeExtension && renewYears && !isExpired) {
        updates.push({
            type: 'renew-name',
            name: nameRecord.name,
            nftId: nameRecord.nftId,
            years: renewYears,
            address,
            ...(applyCoupons ? { couponCodes: coupon } : {}),
        });
    }
    return updates;
}

interface RenewDialogProps {
    name: string;
    setOpen: (bool: boolean) => void;
    onRenew?: () => void;
}

export function RenewNameDialog({ setOpen, name, onRenew }: RenewDialogProps) {
    const queryClient = useQueryClient();
    const iotaClient = useIotaClient();
    const { iotaNamesClient } = useIotaNamesClient();
    const account = useCurrentAccount();
    const { data: nameRecordData, isLoading: isLoadingNameRecord } = useNameRecord(name);
    const { data: config, isLoading: isLoadingConfig } = useNamesConfig();

    // We are sure that only owned names are passed here
    const nameRecord = nameRecordData as
        | Extract<NameRecordData, { type: 'unavailable' }>
        | undefined;

    const renewableYears =
        config && config.coreConfig && nameRecord
            ? getNameRenewableYears(
                  config.coreConfig.max_years,
                  nameRecord.nameRecord.expirationDate,
              )
            : 0;
    const renewOptions = useCalculatePrice(name, renewableYears, false);
    const [renewYears, setRenewYears] = useState<number | undefined>();
    const [coupons, setCoupons] = useState<UserSetCoupon[]>([]);

    const updates = createRenewUpdates({
        nameRecord: nameRecord?.nameRecord,
        renewYears,
        applyCoupons: coupons.length > 0 ? true : false,
        coupon: coupons.length > 0 ? coupons[0]?.code : '',
        address: account?.address,
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
            const currentExpiration = nameRecord?.nameRecord?.expirationDate
                ? new Date(nameRecord.nameRecord.expirationDate)
                : new Date();
            currentExpiration.setFullYear(currentExpiration.getFullYear() + (renewYears || 0));
            const expirationTime = currentExpiration.getTime();

            ampli.renewedName({
                name,
                expirationTime,
                renewYears: renewYears || 0,
            });
            toast.success('Name renewed successfully');
        },
        onError(error) {
            toast.error(getUserFriendlyErrorMessage(error));
        },
    });

    async function handleAddCoupon(couponCode: string) {
        if (coupons.some((c) => c.code === couponCode)) {
            setCoupons((currentCoupons) =>
                currentCoupons.filter((existingCoupon) => existingCoupon.code !== couponCode),
            );
            return;
        }

        try {
            const resolvedCoupon = await iotaNamesClient.resolveCoupon(couponCode);
            if (!resolvedCoupon) throw new Error();
            setCoupons((currentCoupons) => [...currentCoupons, { code: couponCode }]);
        } catch {
            toast.error('Invalid coupon');
        }
    }

    function handleCancelRenewName() {
        setOpen(false);
    }

    function handleYearsChange(years: string) {
        setRenewYears(Number(years));
    }

    const isRenewable = (renewableYears ?? 0) > 0;

    useEffect(() => {
        if (!renewYears && renewOptions.length && renewableYears >= 1) {
            setRenewYears(1);
        }
    }, [renewOptions, renewYears, renewableYears]);

    useEffect(() => {
        const handleErroredCoupon = (erroredCoupon: string) => {
            setCoupons((currentCoupons) =>
                currentCoupons.map((c) =>
                    c.code === erroredCoupon ? { ...c, isInvalid: true } : c,
                ),
            );
        };
        if (updateNameError) {
            const couponRegex = /^Coupon '([^']*)' validation failed/;
            const couponMatch = updateNameError.message.match(couponRegex)?.[1];

            if (couponMatch) {
                handleErroredCoupon(couponMatch);
            }
        }
    }, [updateNameError]);

    const canRenew = nameRecord && updates.length > 0;

    const currentExpirationDate = nameRecord?.nameRecord
        ? formatExpirationDate(nameRecord.nameRecord.expirationDate as Date)
        : null;

    const nextExpirationDate = (() => {
        if (nameRecord?.nameRecord?.expirationDate && renewYears) {
            const expirationDate = new Date(nameRecord.nameRecord.expirationDate);
            expirationDate.setFullYear(expirationDate.getFullYear() + renewYears);
            return formatExpirationDate(expirationDate);
        }
    })();

    const isLoadingData = isLoadingNameRecord || isLoadingConfig;
    const isLoading =
        isLoadingUpdateNameTransaction || isSendingTransaction || isSigning || isLoadingData;

    const disableEdit = isSendingTransaction || isSigning || renewOptions.length === 0;
    const disableSave = isLoading || !canRenew || !renewYears || !updateNameTransaction;
    const cleanName = normalizeIotaName(nameRecord?.nameRecord?.name || name);

    return (
        <Dialog open onOpenChange={setOpen}>
            <DialogContent containerId="overlay-portal-container" position={DialogPosition.Right}>
                <Header title="Renew Name" onClose={() => setOpen(false)} />
                <DialogBody>
                    <div className="flex flex-col justify-between h-full items-center">
                        <div className="flex flex-col w-full gap-y-md">
                            <Panel bgColor="bg-names-neutral-12" data-amp-mask>
                                <div className="px-md py-lg">
                                    <span className="text-names-neutral-100 text-headline-sm break-words">
                                        {cleanName}
                                    </span>
                                </div>
                            </Panel>
                            {isRenewable && !isLoadingData && (
                                <div className="relative">
                                    <Select
                                        options={renewOptions}
                                        value={renewYears?.toString()}
                                        onValueChange={handleYearsChange}
                                        disabled={disableEdit}
                                    />
                                </div>
                            )}
                            {renewOptions.length === 0 && !isLoadingData && (
                                <InfoBox
                                    type={InfoBoxType.Warning}
                                    icon={<Warning />}
                                    title="Renewal Limit Reached"
                                    style={InfoBoxStyle.Default}
                                    supportingText={`This name has already been extended to the maximum allowed period of ${config?.coreConfig?.max_years} years. You'll be able to renew it again once it gets closer to its expiration date`}
                                />
                            )}
                            {isRenewable && (
                                <div className="flex flex-col">
                                    <CouponInputSelection
                                        coupons={coupons}
                                        disabled={isLoading}
                                        onAddCoupon={handleAddCoupon}
                                    />
                                </div>
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
                                {canRenew && !!renewYears && (
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
