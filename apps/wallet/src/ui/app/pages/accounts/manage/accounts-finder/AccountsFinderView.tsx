// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search } from '@iota/ui-icons';
import {
    LoadingIndicator,
    AccountBalanceItem,
    VerifyPasswordModal,
    ConnectLedgerModal,
    useIotaLedgerClient,
} from '_components';
import {
    AccountSourceType,
    type AccountSourceSerializedUI,
} from '_src/background/account-sources/AccountSource';
import { AccountType } from '_src/background/accounts/Account';
import { type SourceStrategyToFind } from '_src/shared/messaging/messages/payloads/accounts-finder';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { getKey } from '_src/ui/app/helpers/accounts';
import { getLedgerConnectionErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { useAccountsFinder } from '_src/ui/app/hooks/useAccountsFinder';
import { useUnlockMutation } from '_src/ui/app/hooks/useUnlockMutation';
import { Button } from '_src/ui/app/shared/ButtonUI';
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
                icon: <Search className="h-6 w-6" />,
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
            icon: <Search className="h-6 w-6" />,
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
                            variant="outline"
                            size="tall"
                            text="Unlock Ledger"
                            onClick={unlockLedger}
                        />
                    ) : isLocked ? (
                        <Button
                            variant="outline"
                            size="tall"
                            text="Verify password"
                            onClick={verifyPassword}
                        />
                    ) : (
                        <>
                            <Button
                                variant="outline"
                                size="tall"
                                text={searchOptions.text}
                                after={searchOptions.icon}
                                onClick={runAccountsFinder}
                                disabled={isSearchOngoing}
                            />

                            <div className="flex flex-row gap-2">
                                <Button
                                    variant="outline"
                                    size="tall"
                                    text="Skip"
                                    disabled={isSearchOngoing}
                                />
                                <Button
                                    variant="outline"
                                    size="tall"
                                    text="Continue"
                                    disabled={isSearchOngoing}
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
