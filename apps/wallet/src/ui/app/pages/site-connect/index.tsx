// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    AccountItemApproveConnection,
    AccountMultiSelectWithControls,
    Loading,
    SectionHeader,
    UserApproveContainer,
} from '_components';
import { useAppDispatch, useAppSelector, useAccountGroups, useActiveAccount } from '_hooks';
import type { RootState } from '_src/ui/app/redux/rootReducer';
import { permissionsSelectors, respondToPermissionRequest } from '_redux/slices/permissions';
import { type SerializedUIAccount } from '_src/background/accounts/account';
import { ampli } from '_src/shared/analytics/ampli';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useParams } from 'react-router-dom';
import { PageMainLayoutTitle } from '../../shared/page-main-layout/PageMainLayoutTitle';
import { InfoBox, InfoBoxStyle, InfoBoxType } from '@iota/apps-ui-kit';
import { Warning, Info } from '@iota/apps-ui-icons';

export function SiteConnectPage() {
    const { requestID } = useParams();
    const permissionsInitialized = useAppSelector(({ permissions }) => permissions.initialized);
    const loading = !permissionsInitialized;
    const permissionSelector = useMemo(
        () => (state: RootState) =>
            requestID ? permissionsSelectors.selectById(state, requestID) : null,
        [requestID],
    );
    const dispatch = useAppDispatch();
    const permissionRequest = useAppSelector(permissionSelector);
    const activeAccount = useActiveAccount();
    const accountGroups = useAccountGroups();
    const accounts = accountGroups.list();
    const unlockedAccounts = accounts.filter((account) => !account.isLocked);
    const lockedAccounts = accounts.filter((account) => account.isLocked);
    const [accountsToConnect, setAccountsToConnect] = useState<SerializedUIAccount[]>(() =>
        activeAccount && !activeAccount.isLocked ? [activeAccount] : [],
    );
    const handleOnSubmit = useCallback(
        async (allowed: boolean) => {
            if (requestID && accountsToConnect && permissionRequest) {
                await dispatch(
                    respondToPermissionRequest({
                        id: requestID,
                        accounts: allowed
                            ? accountsToConnect.map((account) => account.address)
                            : [],
                        allowed,
                    }),
                );
                ampli.respondedToConnectionRequest({
                    applicationName: permissionRequest.name,
                    applicationUrl: permissionRequest.origin,
                    approvedConnection: allowed,
                });
                window.close();
            }
        },
        [requestID, accountsToConnect, permissionRequest, dispatch],
    );
    useEffect(() => {
        if (!loading && !permissionRequest) {
            window.close();
        }
    }, [loading, permissionRequest]);

    const parsedOrigin = useMemo(
        () => (permissionRequest ? new URL(permissionRequest.origin) : null),
        [permissionRequest],
    );

    const isSecure = parsedOrigin?.protocol === 'https:';
    const [displayWarning, setDisplayWarning] = useState(!isSecure);

    const handleHideWarning = useCallback(
        async (allowed: boolean) => {
            if (allowed) {
                setDisplayWarning(false);
            } else {
                await handleOnSubmit(false);
            }
        },
        [handleOnSubmit],
    );

    useEffect(() => {
        setDisplayWarning(!isSecure);
    }, [isSecure]);
    return (
        <Loading loading={loading}>
            {permissionRequest &&
                (displayWarning ? (
                    <UserApproveContainer
                        origin={permissionRequest.origin}
                        originFavIcon={permissionRequest.favIcon}
                        headerTitle="Insecure Website"
                        approveTitle="Continue"
                        rejectTitle="Reject"
                        onSubmit={handleHideWarning}
                        isWarning
                        addressHidden
                        blended
                    >
                        <PageMainLayoutTitle title="Insecure Website" />
                        <div className="flex flex-col gap-lg">
                            <InfoBox
                                title="Your connection is insecure"
                                supportingText="Proceed at your own risk."
                                type={InfoBoxType.Warning}
                                style={InfoBoxStyle.Elevated}
                                icon={<Warning />}
                            />
                            <div className="flex flex-col gap-xs">
                                <span className="text-label-lg text-neutral-60">
                                    Connecting your wallet to this site could expose your data to
                                    attackers.
                                </span>
                                <span className="text-label-lg text-neutral-60">
                                    If you don't have confidence in this site, reject the
                                    connection.
                                </span>
                            </div>
                        </div>
                    </UserApproveContainer>
                ) : (
                    <UserApproveContainer
                        headerTitle="Approve Connection"
                        origin={permissionRequest.origin}
                        originFavIcon={permissionRequest.favIcon}
                        permissions={permissionRequest.permissions}
                        approveTitle="Connect"
                        rejectTitle="Reject"
                        onSubmit={handleOnSubmit}
                        approveDisabled={!accountsToConnect.length}
                        blended
                    >
                        <div className="flex flex-col gap-md">
                            {unlockedAccounts.length > 0 ? (
                                <AccountMultiSelectWithControls
                                    selectedAccountIDs={accountsToConnect.map(
                                        (account) => account.id,
                                    )}
                                    accounts={unlockedAccounts ?? []}
                                    onChange={(value) => {
                                        setAccountsToConnect(
                                            value.map((id) => accounts.find((a) => a.id === id)!),
                                        );
                                    }}
                                />
                            ) : (
                                <InfoBox
                                    icon={<Info />}
                                    style={InfoBoxStyle.Elevated}
                                    type={InfoBoxType.Default}
                                    title="All accounts are currently locked. Unlock accounts to connect."
                                />
                            )}
                            {lockedAccounts?.length > 0 && (
                                <div className="flex flex-col gap-3">
                                    <SectionHeader title="Locked & Unavailable" />
                                    {lockedAccounts?.map((account) => (
                                        <AccountItemApproveConnection
                                            key={account.id}
                                            account={account}
                                        />
                                    ))}
                                </div>
                            )}
                        </div>
                    </UserApproveContainer>
                ))}
        </Loading>
    );
}
