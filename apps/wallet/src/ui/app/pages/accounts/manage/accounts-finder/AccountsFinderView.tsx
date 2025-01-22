// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search } from '@iota/apps-ui-icons';
import { Button, ButtonType, ButtonSize, LoadingIndicator } from '@iota/apps-ui-kit';
import {
    AccountBalanceItem,
    VerifyPasswordModal,
    ConnectLedgerModal,
    useIotaLedgerClient,
} from '_components';
import {
    AccountSourceType,
    type AccountSourceSerializedUI,
} from '_src/background/account-sources/accountSource';
import { AccountType } from '_src/background/accounts/account';
import { type SourceStrategyToFind } from '_src/shared/messaging/messages/payloads/accounts-finder';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { getKey, getLedgerConnectionErrorMessage } from '_src/ui/app/helpers';
import { useAccountSources, useAccounts, useUnlockMutation, useAccountsFinder } from '_hooks';
import { useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import { useParams } from 'react-router-dom';

function getAccountSourceType(
    accountSource?: AccountSourceSerializedUI,
): AllowedAccountSourceTypes {
    switch (accountSource?.type) {
        case AccountSourceType.Mnemonic:
            return AllowedAccountSourceTypes.MnemonicDerived;
        case AccountSourceType.Seed:
            return AllowedAccountSourceTypes.SeedDerived;
        default:
            return AllowedAccountSourceTypes.LedgerDerived;
    }
}

enum SearchPhase {
    Ready, // initialized and ready to start
    Ongoing, // search ongoing
    Idle, // search has finished and is idle, ready to start again
}

export function AccountsFinderView(): JSX.Element {
    const { accountSourceId } = useParams();
    const { data: accountSources } = useAccountSources();
    const { data: accounts } = useAccounts();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceId);
    const accountSourceType = getAccountSourceType(accountSource);
    const [password, setPassword] = useState('');
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const [searchPhase, setSearchPhase] = useState<SearchPhase>(SearchPhase.Ready);
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(false);
    const ledgerIotaClient = useIotaLedgerClient();
    const unlockAccountSourceMutation = useUnlockMutation();
    const sourceStrategy: SourceStrategyToFind = useMemo(
        () =>
            accountSourceType == AllowedAccountSourceTypes.LedgerDerived
                ? {
                      type: 'ledger',
                      password,
                  }
                : {
                      type: 'software',
                      sourceID: accountSourceId!,
                  },
        [password, accountSourceId, accountSourceType],
    );
    const { find } = useAccountsFinder({
        accountSourceType,
        sourceStrategy,
    });

    function unlockLedger() {
        setConnectLedgerModalOpen(true);
    }

    function verifyPassword() {
        setPasswordModalVisible(true);
    }

    async function runAccountsFinder() {
        try {
            setSearchPhase(SearchPhase.Ongoing);
            await find();
        } finally {
            setSearchPhase(SearchPhase.Idle);
        }
    }

    const persistedAccounts = accounts?.filter((acc) => getKey(acc) === accountSourceId);
    const isLocked =
        accountSource?.isLocked || (accountSourceId === AccountType.LedgerDerived && !password);
    const isLedgerLocked =
        accountSourceId === AccountType.LedgerDerived && !ledgerIotaClient.iotaLedgerClient;

    const searchOptions = (() => {
        if (searchPhase === SearchPhase.Ready) {
            return {
                text: 'Search',
                icon: <Search className="h-4 w-4" />,
            };
        }
        if (searchPhase === SearchPhase.Ongoing) {
            return {
                text: '',
                icon: <LoadingIndicator />,
            };
        }
        return {
            text: 'Search again',
            icon: <Search className="h-4 w-4" />,
        };
    })();

    const isSearchOngoing = searchPhase === SearchPhase.Ongoing;

    return (
        <>
            <div className="flex h-full flex-1 flex-col justify-between">
                <div className="flex h-96 flex-col gap-4 overflow-y-auto">
                    {persistedAccounts?.map((account) => {
                        return <AccountBalanceItem key={account.id} account={account} />;
                    })}
                </div>
                <div className="flex flex-col gap-2">
                    {isLedgerLocked ? (
                        <Button
                            size={ButtonSize.Small}
                            type={ButtonType.Secondary}
                            text="Unlock Ledger"
                            onClick={unlockLedger}
                            fullWidth
                        />
                    ) : isLocked ? (
                        <Button
                            type={ButtonType.Secondary}
                            size={ButtonSize.Small}
                            text="Verify password"
                            onClick={verifyPassword}
                            fullWidth
                        />
                    ) : (
                        <>
                            <Button
                                size={ButtonSize.Small}
                                type={ButtonType.Secondary}
                                text={searchOptions.text}
                                icon={searchOptions.icon}
                                iconAfterText
                                onClick={runAccountsFinder}
                                disabled={isSearchOngoing}
                                fullWidth
                            />

                            <div className="flex flex-row gap-2">
                                <Button
                                    size={ButtonSize.Small}
                                    type={ButtonType.Secondary}
                                    text="Skip"
                                    disabled={isSearchOngoing}
                                    fullWidth
                                />
                                <Button
                                    size={ButtonSize.Small}
                                    text="Continue"
                                    disabled={isSearchOngoing}
                                    fullWidth
                                />
                            </div>
                        </>
                    )}
                </div>
            </div>
            {isPasswordModalVisible ? (
                <VerifyPasswordModal
                    open
                    onVerify={async (password) => {
                        if (accountSourceType === AllowedAccountSourceTypes.LedgerDerived) {
                            // for ledger
                            setPassword(password);
                        } else if (accountSourceId) {
                            // unlock software account sources
                            await unlockAccountSourceMutation.mutateAsync({
                                id: accountSourceId,
                                password,
                            });
                        }

                        setPasswordModalVisible(false);
                    }}
                    onClose={() => setPasswordModalVisible(false)}
                />
            ) : null}
            {isConnectLedgerModalOpen && (
                <ConnectLedgerModal
                    onClose={() => {
                        setConnectLedgerModalOpen(false);
                    }}
                    onError={(error) => {
                        setConnectLedgerModalOpen(false);
                        toast.error(
                            getLedgerConnectionErrorMessage(error) || 'Something went wrong.',
                        );
                    }}
                    onConfirm={() => {
                        setConnectLedgerModalOpen(false);
                        setPasswordModalVisible(true);
                    }}
                />
            )}
        </>
    );
}
