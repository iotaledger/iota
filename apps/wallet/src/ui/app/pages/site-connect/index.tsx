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
import { useAppDispatch, useAppSelector } from '_hooks';
import type { RootState } from '_redux/RootReducer';
import { permissionsSelectors, respondToPermissionRequest } from '_redux/slices/permissions';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import { ampli } from '_src/shared/analytics/ampli';
import { useCallback, useEffect, useMemo, useState } from 'react';
import { useParams } from 'react-router-dom';

import { useAccountGroups } from '../../hooks/useAccountGroups';
import { useActiveAccount } from '../../hooks/useActiveAccount';
import { InfoBox, InfoBoxStyle, InfoBoxType, Title, TitleSize } from '@iota/apps-ui-kit';
import { Info } from '@iota/ui-icons';

function SiteConnectPage() {
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
                        <div className="flex w-full flex-col flex-nowrap gap-xs rounded-xl bg-neutral-96 pb-md">
                            <div className="flex items-center">
                                <Title
                                    title="Your Connection is Not Secure"
                                    size={TitleSize.Small}
                                />
                            </div>
                            <div className="px-md">
                                If you connect your wallet to this site your data could be exposed
                                to attackers. Click **Reject** if you don't trust this site.
                                <br />
                                <br />
                                Continue at your own risk.
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
                        <div className="flex flex-col gap-8 py-6">
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
                                            showLock
                                            account={account}
                                            disabled={account.isLocked}
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

export default SiteConnectPage;
