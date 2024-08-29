// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import cn from 'clsx';
import type { ReactNode } from 'react';
import { useCallback, useMemo, useState } from 'react';
import { Button, ButtonType, Header } from '@iota/apps-ui-kit';
import { Loader } from '@iota/ui-icons';

import { useAccountByAddress } from '../../hooks/useAccountByAddress';
import { DAppInfoCard, UnlockAccountButton } from '_components';

interface UserApproveContainerProps {
    children: ReactNode | ReactNode[];
    origin: string;
    originFavIcon?: string;
    headerTitle?: string;
    rejectTitle: string;
    approveTitle: string;
    approveDisabled?: boolean;
    approveLoading?: boolean;
    onSubmit: (approved: boolean) => Promise<void>;
    isWarning?: boolean;
    addressHidden?: boolean;
    address?: string | null;
    scrollable?: boolean;
    blended?: boolean;
    permissions?: PermissionType[];
    checkAccountLock?: boolean;
}

export function UserApproveContainer({
    origin,
    originFavIcon,
    children,
    headerTitle,
    rejectTitle,
    approveTitle,
    approveDisabled = false,
    approveLoading = false,
    onSubmit,
    isWarning,
    addressHidden = false,
    address,
    permissions,
    checkAccountLock,
}: UserApproveContainerProps) {
    const [submitting, setSubmitting] = useState(false);
    const handleOnResponse = useCallback(
        async (allowed: boolean) => {
            setSubmitting(true);
            await onSubmit(allowed);
            setSubmitting(false);
        },
        [onSubmit],
    );
    const { data: selectedAccount } = useAccountByAddress(address);
    const parsedOrigin = useMemo(() => new URL(origin), [origin]);
    return (
        <div className="flex h-full flex-1 flex-col flex-nowrap">
            {headerTitle && <Header title={headerTitle} titleCentered />}
            <div className="flex flex-1 flex-col pb-0">
                <DAppInfoCard
                    name={parsedOrigin.host}
                    url={origin}
                    permissions={permissions}
                    iconUrl={originFavIcon}
                    connectedAddress={!addressHidden && address ? address : undefined}
                />
                <div className="bg-hero-darkest/5 flex flex-1 flex-col px-6">{children}</div>
            </div>
            <div className="sticky bottom-0 z-10">
                <div
                    className={cn('flex items-center bg-white p-md pt-sm', {
                        'flex-row-reverse': isWarning,
                    })}
                >
                    {!checkAccountLock || !selectedAccount?.isLocked ? (
                        <div className="flex w-full gap-md">
                            <Button
                                type={ButtonType.Secondary}
                                onClick={() => {
                                    handleOnResponse(false);
                                }}
                                disabled={submitting}
                                text={rejectTitle}
                                fullWidth
                            />
                            <Button
                                fullWidth
                                type={isWarning ? ButtonType.Secondary : ButtonType.Primary}
                                onClick={() => {
                                    handleOnResponse(true);
                                }}
                                icon={
                                    (submitting || approveLoading) && (
                                        <Loader className="animate-spin" />
                                    )
                                }
                                disabled={approveDisabled}
                                text={approveTitle}
                            />
                        </div>
                    ) : (
                        <UnlockAccountButton account={selectedAccount} title="Unlock to Approve" />
                    )}
                </div>
            </div>
        </div>
    );
}
