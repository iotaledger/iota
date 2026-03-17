// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    useAutoLockMinutes,
    useBackgroundClient,
    useStorageMigrationStatus,
    useAccounts,
    useAppDispatch,
    useAppSelector,
} from './hooks';
import { setNavVisibility } from '_redux/slices/app';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { type LedgerAccountsPublicKeys } from '_src/shared/messaging/messages/payloads/methodPayload';
import { toBase64 } from '@iota/iota-sdk/utils';
import { useEffect, useMemo } from 'react';
import { Navigate, Route, Routes, useLocation } from 'react-router-dom';
import { throttle } from 'throttle-debounce';
import { useIotaLedgerClient } from './components/ledger/IotaLedgerClientProvider';
import { AccountsPage } from './pages/accounts/AccountsPage';
import { AddAccountPage } from './pages/accounts/AddAccountPage';
import { BackupMnemonicPage } from './pages/accounts/BackupMnemonicPage';
import { ExportAccountPage } from './pages/accounts/ExportAccountPage';
import { ExportPassphrasePage } from './pages/accounts/ExportPassphrasePage';
import { ExportSeedPage } from './pages/accounts/ExportSeedPage';
import { ImportLedgerAccountsPage } from './pages/accounts/ImportLedgerAccountsPage';
import { ImportPassphrasePage } from './pages/accounts/ImportPassphrasePage';
import { ImportPrivateKeyPage } from './pages/accounts/ImportPrivateKeyPage';
import { ImportSeedPage } from './pages/accounts/ImportSeedPage';
import { ManageAccountsPage } from './pages/accounts/manage/ManageAccountsPage';
import { ProtectAccountPage } from './pages/accounts/ProtectAccountPage';
import { WelcomePage } from './pages/accounts/WelcomePage';
import { ApprovalRequestPage } from './pages/approval-request';
import {
    HomePage,
    AppsPage,
    AssetsPage,
    CoinsSelectorPage,
    NFTDetailsPage,
    NftTransferPage,
    KioskDetailsPage,
    ReceiptPage,
    TransactionBlocksPage,
    TransferCoinPage,
} from './pages/home';
import { TokenDetailsPage } from './pages/home/tokens/TokenDetailsPage';
import { RestrictedPage } from './pages/restricted';
import { SiteConnectPage } from './pages/site-connect';
import { ExtensionViewType } from './redux/slices/app/appType';
import { StakingPage } from './staking/home';
import { StorageMigrationPage } from './pages/StorageMigrationPage';
import { AccountsFinderPage } from './pages/accounts/manage/accounts-finder/AccountsFinderPage';
import { AccountsFinderIntroPage } from './pages/accounts/manage/accounts-finder/AccountsFinderIntroPage';
import { CreateNewPasskey } from './pages/accounts/CreateNewPasskey';
import { ImportPasskeyPage } from './pages/accounts/ImportPasskeyPage';
import { ImportKeystone } from './pages/accounts/ImportKeystone';
import { CreateNewWallet } from './pages/accounts/CreateNewWallet';
import { ImportExistingWallet } from './pages/accounts/ImportExistingWallet';

const HIDDEN_MENU_PATHS = [
    '/nft-details',
    '/nft-transfer',
    '/receipt',
    '/send',
    '/send/select',
    '/apps/disconnectapp',
];

const NOTIFY_USER_ACTIVE_INTERVAL = 5 * 1000; // 5 seconds

export function App() {
    const dispatch = useAppDispatch();
    const isPopup = useAppSelector(
        (state) => state.app.extensionViewType === ExtensionViewType.Popup,
    );
    useEffect(() => {
        document.body.classList.remove('app-initializing');
    }, [isPopup]);
    const location = useLocation();
    useEffect(() => {
        const menuVisible = !HIDDEN_MENU_PATHS.some((aPath) => location.pathname.startsWith(aPath));
        dispatch(setNavVisibility(menuVisible));
    }, [location, dispatch]);

    const { data: accounts } = useAccounts();
    const allLedgerWithoutPublicKey = useMemo(
        () =>
            accounts?.filter(isLedgerAccountSerializedUI).filter(({ publicKey }) => !publicKey) ||
            [],
        [accounts],
    );
    const backgroundClient = useBackgroundClient();
    const { connectToLedger, iotaLedgerClient } = useIotaLedgerClient();
    useEffect(() => {
        // update ledger accounts without the public key
        (async () => {
            if (allLedgerWithoutPublicKey.length) {
                try {
                    if (!iotaLedgerClient) {
                        await connectToLedger();
                        return;
                    }
                    const publicKeysToStore: LedgerAccountsPublicKeys = [];
                    for (const { derivationPath, id } of allLedgerWithoutPublicKey) {
                        if (derivationPath) {
                            try {
                                const { publicKey } =
                                    await iotaLedgerClient.getPublicKey(derivationPath);
                                publicKeysToStore.push({
                                    accountID: id,
                                    publicKey: toBase64(publicKey),
                                });
                            } catch (e) {
                                // do nothing
                            }
                        }
                    }
                    if (publicKeysToStore.length) {
                        await backgroundClient.storeLedgerAccountsPublicKeys({ publicKeysToStore });
                    }
                } catch (e) {
                    // do nothing
                }
            }
        })();
    }, [allLedgerWithoutPublicKey, iotaLedgerClient, backgroundClient, connectToLedger]);
    const { data } = useAutoLockMinutes();
    const autoLockEnabled = !!data;
    // use mouse move and key down events to detect user activity
    // this is used to adjust the auto-lock timeout
    useEffect(() => {
        if (!autoLockEnabled) {
            return;
        }
        const sendUpdateThrottled = throttle(
            NOTIFY_USER_ACTIVE_INTERVAL,
            () => {
                backgroundClient.notifyUserActive();
            },
            { noTrailing: true },
        );
        document.addEventListener('mousemove', sendUpdateThrottled);
        document.addEventListener('keydown', sendUpdateThrottled);
        return () => {
            document.removeEventListener('mousemove', sendUpdateThrottled);
            document.removeEventListener('keydown', sendUpdateThrottled);
        };
    }, [backgroundClient, autoLockEnabled]);
    // Placeholder check for storage migration.
    // currently hook useStorageMigrationStatus always returns 'ready'
    const storageMigration = useStorageMigrationStatus();
    if (storageMigration.isPending || !storageMigration?.data) {
        return null;
    }
    if (storageMigration.data !== 'ready') {
        return <StorageMigrationPage />;
    }
    return (
        <Routes>
            <Route path="restricted" element={<RestrictedPage />} />
            <Route path="/*" element={<HomePage />}>
                <Route path="apps/*" element={<AppsPage />} />
                <Route path="kiosk" element={<KioskDetailsPage />} />
                <Route path="nft-details" element={<NFTDetailsPage />} />
                <Route path="nft-transfer/:nftId" element={<NftTransferPage />} />
                <Route path="nfts/*" element={<AssetsPage />} />
                <Route path="receipt" element={<ReceiptPage />} />
                <Route path="send" element={<TransferCoinPage />} />
                <Route path="send/select" element={<CoinsSelectorPage />} />
                <Route path="stake/*" element={<StakingPage />} />
                <Route path="tokens/*" element={<TokenDetailsPage />} />
                <Route path="transactions/:status?" element={<TransactionBlocksPage />} />
                <Route path="*" element={<Navigate to="/tokens" replace={true} />} />
            </Route>
            <Route path="accounts/*" element={<AccountsPage />}>
                <Route path="welcome" element={<WelcomePage />} />
                <Route path="add-account" element={<AddAccountPage />} />
                <Route path="create-new" element={<CreateNewWallet />} />
                <Route path="import-existing" element={<ImportExistingWallet />} />
                <Route path="import-ledger-accounts" element={<ImportLedgerAccountsPage />} />
                <Route path="import-passphrase" element={<ImportPassphrasePage />} />
                <Route path="import-private-key" element={<ImportPrivateKeyPage />} />
                <Route path="import-seed" element={<ImportSeedPage />} />
                <Route path="passkey-account" element={<CreateNewPasskey />} />
                <Route path="import-passkey" element={<ImportPasskeyPage />} />
                <Route path="import-keystone" element={<ImportKeystone />} />
                <Route path="manage" element={<ManageAccountsPage />} />
                <Route path="manage/accounts-finder/intro" element={<AccountsFinderIntroPage />} />
                <Route
                    path="manage/accounts-finder/:accountSourceId"
                    element={<AccountsFinderPage />}
                />
                <Route path="protect-account" element={<ProtectAccountPage />} />
                <Route path="backup/:accountSourceID" element={<BackupMnemonicPage />} />
                <Route path="export/:accountID" element={<ExportAccountPage />} />
                <Route
                    path="export/passphrase/:accountSourceID"
                    element={<ExportPassphrasePage />}
                />
                <Route path="export/seed/:accountSourceID" element={<ExportSeedPage />} />
            </Route>
            <Route path="/dapp/*" element={<HomePage disableNavigation />}>
                <Route path="connect/:requestID" element={<SiteConnectPage />} />
                <Route path="approve/:requestID" element={<ApprovalRequestPage />} />
            </Route>
        </Routes>
    );
}
