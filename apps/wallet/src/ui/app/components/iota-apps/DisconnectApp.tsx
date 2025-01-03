// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Overlay, DAppInfoCard, WalletListSelect } from '_components';
import { useAppSelector } from '_hooks';
import { permissionsSelectors } from '_redux/slices/permissions';
import { ampli } from '_src/shared/analytics/ampli';
import { formatAddress } from '@iota/iota-sdk/utils';
import { useMutation } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';
import { toast } from 'react-hot-toast';

import { useBackgroundClient } from '../../hooks/useBackgroundClient';
import { type DAppEntry } from './IotaApp';

import { CircleEmitter } from '@iota/ui-icons';
import { Button, ButtonType } from '@iota/apps-ui-kit';
import { SummaryPanel } from '../SummaryPanel';
import { SummaryListItem } from '../SummaryListItem';
import { DAppPermissionList } from '../DAppPermissionList';

export interface DisconnectAppProps extends Omit<DAppEntry, 'description' | 'tags'> {
    permissionID: string;
    setShowDisconnectApp: (showModal: boolean) => void;
}

export function DisconnectApp({
    name,
    icon,
    link,
    permissionID,
    setShowDisconnectApp,
}: DisconnectAppProps) {
    const [accountsToDisconnect, setAccountsToDisconnect] = useState<string[]>([]);
    const permission = useAppSelector((state) =>
        permissionsSelectors.selectById(state, permissionID),
    );
    useEffect(() => {
        if (permission && !permission.allowed) {
            setShowDisconnectApp(false);
        }
    }, [permission, setShowDisconnectApp]);
    const connectedAccounts = useMemo(
        () => (permission?.allowed && permission.accounts) || [],
        [permission],
    );
    const backgroundClient = useBackgroundClient();
    const disconnectMutation = useMutation({
        mutationFn: async () => {
            const origin = permission?.origin;
            if (!origin) {
                throw new Error('Failed, origin not found');
            }

            await backgroundClient.disconnectApp(origin, accountsToDisconnect);
            await backgroundClient.sendGetPermissionRequests();
            ampli.disconnectedApplication({
                sourceFlow: 'Application page',
                disconnectedAccounts: accountsToDisconnect.length || 1,
                applicationName: permission.name,
                applicationUrl: origin,
            });
        },
        onSuccess: () => {
            toast.success('Disconnected successfully');
            setShowDisconnectApp(false);
        },
        onError: () => toast.error('Disconnect failed'),
    });
    if (!permission) {
        return null;
    }
    return (
        <Overlay showModal setShowModal={setShowDisconnectApp} title="Active Connection">
            <div className="flex max-w-full flex-1 flex-col flex-nowrap items-stretch gap-y-md">
                <DAppInfoCard name={name} iconUrl={icon} url={link} />

                <SummaryPanel
                    title="Permissions requested"
                    body={
                        <div className="px-md">
                            <DAppPermissionList permissions={permission.permissions} />
                        </div>
                    }
                />

                <SummaryPanel
                    title={'Connected Account' + (connectedAccounts.length > 1 ? 's' : '')}
                    body={
                        <div className="px-md">
                            {connectedAccounts.length > 1 ? (
                                <WalletListSelect
                                    visibleValues={connectedAccounts}
                                    values={accountsToDisconnect}
                                    onChange={setAccountsToDisconnect}
                                    disabled={disconnectMutation.isPending}
                                />
                            ) : (
                                <SummaryListItem
                                    icon={
                                        <CircleEmitter className="h-5 w-5 text-neutral-10 dark:text-neutral-92" />
                                    }
                                    text={
                                        connectedAccounts[0]
                                            ? formatAddress(connectedAccounts[0])
                                            : ''
                                    }
                                />
                            )}
                        </div>
                    }
                />

                <div className="sticky bottom-0 flex flex-1 items-end pt-xs">
                    <Button
                        type={ButtonType.Secondary}
                        fullWidth
                        text={
                            connectedAccounts.length === 1
                                ? 'Disconnect'
                                : accountsToDisconnect.length === 0 ||
                                    connectedAccounts.length === accountsToDisconnect.length
                                  ? 'Disconnect All'
                                  : 'Disconnect Selected'
                        }
                        disabled={disconnectMutation.isPending}
                        onClick={() => disconnectMutation.mutate()}
                    />
                </div>
            </div>
        </Overlay>
    );
}
