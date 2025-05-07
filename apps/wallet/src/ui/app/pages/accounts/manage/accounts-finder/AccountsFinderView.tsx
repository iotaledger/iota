// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Info, Search } from '@iota/apps-ui-icons';
import {
    Button,
    ButtonType,
    InfoBox,
    InfoBoxStyle,
    InfoBoxType,
    LoadingIndicator,
} from '@iota/apps-ui-kit';
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
import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { type SourceStrategyToFind } from '_src/shared/messaging/messages/payloads/accounts-finder';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { getKey, getLedgerConnectionErrorMessage } from '_src/ui/app/helpers';
import { useAccountSources, useAccounts, useUnlockMutation, useAccountsFinder } from '_hooks';
import { useMemo, useState } from 'react';
import { toast } from '@iota/core';
import { useNavigate, useParams } from 'react-router-dom';
import { parseDerivationPath } from '_src/background/account-sources/bip44Path';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';

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
    const navigate = useNavigate();
    const { accountSourceId } = useParams();
    const { data: accountSources } = useAccountSources();
    const { data: accounts } = useAccounts();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceId);
    const accountSourceType = getAccountSourceType(accountSource);
    const [password, setPassword] = useState('');
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const [searchPhase, setSearchPhase] = useState<SearchPhase>(SearchPhase.Ready);
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(false);
    const [searchIteration, setSearchIteration] = useState(1);
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
    const { find, progress, isSearching } = useAccountsFinder({
        accountSourceType,
        sourceStrategy,
    });

    console.log(isSearching);

    // Calculate percentage based on progress information
    const calculateProgressPercentage = () => {
        if (progress.searchType === 'breadth' && progress.totalAccounts) {
            // For breadth search, calculate based on account progress
            const totalAccounts =
                progress.totalAccounts -
                (progress.currentAccountIndex -
                    (progress.totalAccounts || 0) +
                    (progress.accountGapLimit || 0));
            return Math.min(100, Math.round((progress.currentAccountIndex / totalAccounts) * 100));
        } else if (progress.searchType === 'depth') {
            // For depth search, show progress based on address index if available
            if (progress.addressGapLimit) {
                return Math.min(
                    100,
                    Math.round((progress.currentAddressIndex / progress.addressGapLimit) * 100),
                );
            }
            // Default to indeterminate progress
            return null;
        }
        return null;
    };

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
            // Increment search iteration when a search completes
            setSearchIteration((prev) => prev + 1);
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
            text: 'Keep searching',
            icon: <Search className="h-4 w-4" />,
        };
    })();

    const isSearchOngoing = searchPhase === SearchPhase.Ongoing;

    function groupAccountsByAccountIndex(
        accounts: SerializedUIAccount[],
    ): Record<number, SerializedUIAccount[]> {
        const groupedAccounts: Record<number, SerializedUIAccount[]> = {};
        accounts.forEach((account) => {
            if (
                isMnemonicSerializedUiAccount(account) ||
                isSeedSerializedUiAccount(account) ||
                isLedgerAccountSerializedUI(account)
            ) {
                const { accountIndex } = parseDerivationPath(account.derivationPath);
                if (!groupedAccounts[accountIndex]) {
                    groupedAccounts[accountIndex] = [];
                }
                groupedAccounts[accountIndex].push(account);
            }
        });
        return groupedAccounts;
    }
    const groupedAccounts = persistedAccounts && groupAccountsByAccountIndex(persistedAccounts);

    return (
        <>
            <div className="flex h-full flex-col justify-between">
                <div className="flex h-[480px] w-full flex-col gap-xs overflow-y-auto">
                    {Object.entries(groupedAccounts || {}).map(([accountIndex, accounts]) => {
                        return (
                            <AccountBalanceItem
                                key={accountIndex}
                                accountIndex={accountIndex}
                                accounts={accounts}
                            />
                        );
                    })}
                </div>
                <div className="flex flex-col gap-xs pt-sm">
                    {isLedgerLocked ? (
                        <Button
                            type={ButtonType.Secondary}
                            text="Unlock Ledger"
                            onClick={unlockLedger}
                            fullWidth
                        />
                    ) : isLocked ? (
                        <Button
                            type={ButtonType.Secondary}
                            text="Verify password"
                            onClick={verifyPassword}
                            fullWidth
                        />
                    ) : (
                        <>
                            {searchOptions.text === 'Keep searching' ? (
                                <InfoBox
                                    supportingText="Some funds or addresses may not appear immediately. Run multiple searches to ensure all assets are located."
                                    icon={<Info />}
                                    type={InfoBoxType.Default}
                                    style={InfoBoxStyle.Elevated}
                                />
                            ) : null}
                            <Button
                                type={ButtonType.Secondary}
                                text={searchOptions.text}
                                icon={searchOptions.icon}
                                iconAfterText
                                onClick={runAccountsFinder}
                                disabled={isSearchOngoing}
                                fullWidth
                            />

                            <div className="flex flex-row gap-xs">
                                <Button
                                    text="Finish"
                                    disabled={isSearchOngoing}
                                    fullWidth
                                    onClick={() => navigate('/tokens')}
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
