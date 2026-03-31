// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountMultiSelectWithControls, Loading, UserApproveContainer } from '_components';
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
import { ExtensionViewType } from '../../redux/slices/app/appType';
import { SidePanel } from '_src/polyfills/sidepanel';
import { resolveApplicationName } from '_src/shared/utils';

export function SiteConnectPage() {
    const { requestID } = useParams();
    const extensionViewType = useAppSelector((state) => state.app.extensionViewType);
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

    const [accountsToConnect, setAccountsToConnect] = useState<SerializedUIAccount[]>(() => {
        const preselectedAccounts = activeAccount && !activeAccount.isLocked ? [activeAccount] : [];

        const previouslyPermittedAccounts = permissionRequest?.accounts.length
            ? accounts.filter((acc) => permissionRequest.accounts.includes(acc.address))
            : [];

        return preselectedAccounts.concat(previouslyPermittedAccounts);
    });

    function handleOnFinish() {
        if (extensionViewType === ExtensionViewType.SidePanel) {
            SidePanel.enableAndGoTo(`${location.pathname}`);
        } else {
            window.close();
        }
    }

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
                const resolvedAppName = resolveApplicationName(
                    permissionRequest.name,
                    permissionRequest.origin,
                );
                ampli.respondedToConnectionRequest({
                    applicationName: resolvedAppName,
                    applicationUrl: permissionRequest.origin,
                    approvedConnection: allowed,
                });
                handleOnFinish();
            }
        },
        [requestID, accountsToConnect, permissionRequest, dispatch],
    );
    useEffect(() => {
        if (!loading && !permissionRequest) {
            handleOnFinish();
        }
    }, [loading, permissionRequest]);

    const parsedOrigin = useMemo(
        () => (permissionRequest ? new URL(permissionRequest.origin) : null),
        [permissionRequest],
    );

    const isSecure = parsedOrigin?.protocol === 'https:';
    const [warningDismissed, setWarningDismissed] = useState(false);
    const displayWarning = !isSecure && !warningDismissed;

    const handleHideWarning = useCallback(
        async (allowed: boolean) => {
            if (allowed) {
                setWarningDismissed(true);
            } else {
                await handleOnSubmit(false);
            }
        },
        [handleOnSubmit],
    );

    useEffect(() => {
        if (permissionRequest) {
            const resolvedAppName = resolveApplicationName(
                permissionRequest.name,
                permissionRequest.origin,
            );
            ampli.startedDappConnection({
                applicationName: resolvedAppName,
                applicationUrl: permissionRequest.origin,
            });
        }
    }, [permissionRequest]);

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
                                <span className="text-label-lg text-iota-neutral-60">
                                    Connecting your wallet to this site could expose your data to
                                    attackers.
                                </span>
                                <span className="text-label-lg text-iota-neutral-60">
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
                            {accounts.length > 0 ? (
                                <AccountMultiSelectWithControls
                                    selectedAccountIDs={accountsToConnect.map(
                                        (account) => account.id,
                                    )}
                                    accounts={accounts ?? []}
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
                        </div>
                    </UserApproveContainer>
                ))}
        </Loading>
    );
}
