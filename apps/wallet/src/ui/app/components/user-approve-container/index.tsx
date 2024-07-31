// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type PermissionType } from '_src/shared/messaging/messages/payloads/permissions';
import cn from 'clsx';
import { useCallback, useMemo, useState } from 'react';
import type { ReactNode } from 'react';

import { useAccountByAddress } from '../../hooks/useAccountByAddress';
import { Button } from '../../shared/ButtonUI';
import { UnlockAccountButton } from '../accounts/UnlockAccountButton';
import { DAppInfoCard } from '../DAppInfoCard';

interface UserApproveContainerProps {
    children: ReactNode | ReactNode[];
    origin: string;
    originFavIcon?: string;
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
            <div className="sticky bottom-0">
                <div
                    className={cn(
                        'bg-hero-darkest/5 flex items-center gap-2.5 px-5 py-4 backdrop-blur-lg',
                        {
                            'flex-row-reverse': isWarning,
                        },
                    )}
                >
                    {!checkAccountLock || !selectedAccount?.isLocked ? (
                        <>
                            <Button
                                size="tall"
                                variant="secondary"
                                onClick={() => {
                                    handleOnResponse(false);
                                }}
                                disabled={submitting}
                                text={rejectTitle}
                            />
                            <Button
                                size="tall"
                                variant={isWarning ? 'secondary' : 'primary'}
                                onClick={() => {
                                    handleOnResponse(true);
                                }}
                                disabled={approveDisabled}
                                loading={submitting || approveLoading}
                                text={approveTitle}
                            />
                        </>
                    ) : (
                        <UnlockAccountButton account={selectedAccount} title="Unlock to Approve" />
                    )}
                </div>
            </div>
        </div>
    );
}
