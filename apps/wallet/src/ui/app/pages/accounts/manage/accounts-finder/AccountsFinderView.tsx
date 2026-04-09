// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search } from '@iota/apps-ui-icons';
import { Button, ButtonType, LoadingIndicator } from '@iota/apps-ui-kit';
import {
    AccountBalanceItem,
    VerifyPasswordModal,
    ConnectLedgerModal,
    useIotaLedgerClient,
    OffBalanceAddressesInfo,
} from '_components';
import {
    AccountSourceType,
    type AccountSourceSerializedUI,
} from '_src/background/account-sources/accountSource';
import { AccountType, type SerializedUIAccount } from '_src/background/accounts/account';
import { type SourceStrategyToFind } from '_src/shared/messaging/messages/payloads/accounts-finder';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { getSourceId, getLedgerConnectionErrorMessage, isFirstAccount } from '_src/ui/app/helpers';
import {
    useAccountSources,
    useAccounts,
    useAccountsFinder,
    useGetOwnedObjectsMultipleAddresses,
    useGetSharedObjectsMultipleAddresses,
} from '_hooks';
import { useMemo, useState } from 'react';
import {
    STARDUST_BASIC_OUTPUT_TYPE,
    STARDUST_NFT_OUTPUT_TYPE,
    TIMELOCK_IOTA_TYPE,
    TIMELOCK_STAKED_TYPE,
    toast,
} from '@iota/core';
import { useNavigate, useParams, useSearchParams } from 'react-router-dom';
import { parseDerivationPath } from '_src/background/account-sources/bip44Path';
import { isMnemonicSerializedUiAccount } from '_src/background/accounts/mnemonicAccount';
import { isSeedSerializedUiAccount } from '_src/background/accounts/seedAccount';
import { isLedgerAccountSerializedUI } from '_src/background/accounts/ledgerAccount';
import { MigrationDialog } from '../../../home/tokens/MigrationDialog';
import { SupplyIncreaseVestingStakingDialog } from '../../../home/tokens/SupplyIncreaseVestingStakingDialog';
import { ampli } from '_src/shared/analytics/ampli';
import {
    ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE,
    AmpliAccountOrigin,
    AmpliSourceFlow,
} from '_src/shared/analytics';
import type { AddedAccountsProperties } from '_src/shared/analytics/ampli';

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

/**
 * Maps account source to Amplitude accountType for balanceFinderUsed event.
 * Uses ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE for consistency across the app.
 */
function getAmplitudeAccountType(accountSource?: AccountSourceSerializedUI): string {
    let ampliAccountSourceType: string | undefined = '';
    if (accountSource) {
        switch (accountSource.type) {
            case AccountSourceType.Mnemonic: {
                ampliAccountSourceType =
                    ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE[AccountType.MnemonicDerived];
                break;
            }
            case AccountSourceType.Seed: {
                ampliAccountSourceType =
                    ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE[AccountType.SeedDerived];
                break;
            }
            case AccountSourceType.Keystone: {
                ampliAccountSourceType =
                    ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE[AccountType.KeystoneDerived];
                break;
            }
            default: {
                ampliAccountSourceType =
                    ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE[AccountType.LedgerDerived];
                break;
            }
        }
    }
    return ampliAccountSourceType || 'unknown';
}

enum SearchPhase {
    Ready, // initialized and ready to start
    Ongoing, // search ongoing
    Idle, // search has finished and is idle, ready to start again
}

export function AccountsFinderView(): JSX.Element {
    const navigate = useNavigate();
    const { accountSourceId } = useParams();
    const [searchParams] = useSearchParams();
    const mainPublicKey = searchParams.get('mainPublicKey');
    const { data: accountSources } = useAccountSources();
    const { data: accounts } = useAccounts();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceId);
    const accountSourceType = getAccountSourceType(accountSource);
    const [password, setPassword] = useState('');
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const [searchPhase, setSearchPhase] = useState<SearchPhase>(SearchPhase.Ready);
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(false);
    const [totalCheckedAddresses, setTotalCheckedAddresses] = useState(1);
    const [dialogVestingOpen, setDialogVestingOpen] = useState(false);
    const [dialogMigrationOpen, setDialogMigrationOpen] = useState(false);

    const ledgerIotaClient = useIotaLedgerClient();
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
        onDerivationPathChecked: ({ totalCheckedAddresses }) => {
            setTotalCheckedAddresses(totalCheckedAddresses);
        },
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
            ampli.usedBalanceFinder({
                accountType: getAmplitudeAccountType(accountSource),
            });
            const numberOfAccountsCreated = await find();

            // Fire accountsAdded event if accounts were created
            if (numberOfAccountsCreated > 0) {
                const accountType: AddedAccountsProperties['accountType'] =
                    getAmplitudeAccountType(accountSource);

                ampli.addedAccounts({
                    accountType,
                    accountOrigin: AmpliAccountOrigin.Import,
                    numberOfAccounts: numberOfAccountsCreated,
                    isFirstAccount: isFirstAccount(accounts),
                    sourceFlow: AmpliSourceFlow.BalanceFinder,
                });
            }
        } finally {
            setSearchPhase(SearchPhase.Idle);
        }
    }

    function filterAccounts(acc: SerializedUIAccount): boolean {
        if (accountSourceType === AllowedAccountSourceTypes.LedgerDerived && mainPublicKey) {
            return isLedgerAccountSerializedUI(acc) && acc.mainPublicKey === mainPublicKey;
        } else {
            return getSourceId(acc) === accountSourceId;
        }
    }

    const persistedAccounts = accounts?.filter(filterAccounts);
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
                text: 'Searching',
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

    const findingResultText = `Scanned ${totalCheckedAddresses} addresses`;
    const allAddresses = persistedAccounts?.map((a) => a.address) ?? [];

    const { data: ownedVestingObjects } = useGetOwnedObjectsMultipleAddresses(
        allAddresses,
        { MatchAny: [{ StructType: TIMELOCK_IOTA_TYPE }, { StructType: TIMELOCK_STAKED_TYPE }] },
        10,
    );
    const { data: stardustOwned } = useGetOwnedObjectsMultipleAddresses(
        allAddresses,
        {
            MatchAny: [
                { StructType: STARDUST_BASIC_OUTPUT_TYPE },
                { StructType: STARDUST_NFT_OUTPUT_TYPE },
            ],
        },
        1,
    );
    const { data: stardustShared } = useGetSharedObjectsMultipleAddresses(allAddresses, 1);

    const hasVestingObjects = ownedVestingObjects?.pages?.some((p) =>
        p.some((d) => d.data.length > 0),
    );

    const hasMigrationObjects =
        stardustOwned?.pages?.some((p) => p.some((d) => d.data.length > 0)) ||
        stardustShared?.pages?.some((p) =>
            p.some((d) => d.nftOutputs.length > 0 || d.basicOutputs.length > 0),
        );
    return (
        <>
            <div className="flex h-full flex-col justify-between">
                {(hasVestingObjects || hasMigrationObjects) && (
                    <>
                        <OffBalanceAddressesInfo
                            hasVesting={!!hasVestingObjects}
                            hasMigration={!!hasMigrationObjects}
                            onOpenVestingInfo={() => setDialogVestingOpen(true)}
                            onOpenMigrationInfo={() => setDialogMigrationOpen(true)}
                        />
                        <span className="my-sm text-center text-title-md text-iota-neutral-10 dark:text-iota-neutral-92">
                            Found Addresses
                        </span>
                    </>
                )}
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
                    {(searchOptions.text === 'Keep searching' || isSearchOngoing) && (
                        <span className="text-center text-iota-neutral-40 dark:text-iota-neutral-60">
                            {findingResultText}
                        </span>
                    )}
                    <div className="flex flex-row gap-xs">
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
                                <Button
                                    text="Close"
                                    type={ButtonType.Secondary}
                                    fullWidth
                                    onClick={() => navigate('/tokens')}
                                />
                                <Button
                                    text={searchOptions.text}
                                    icon={searchOptions.icon}
                                    iconAfterText
                                    onClick={runAccountsFinder}
                                    disabled={isSearchOngoing}
                                    fullWidth
                                />
                            </>
                        )}
                    </div>
                </div>
            </div>
            {isPasswordModalVisible ? (
                <VerifyPasswordModal
                    open
                    onVerify={async (password) => {
                        if (accountSourceType === AllowedAccountSourceTypes.LedgerDerived) {
                            // for ledger
                            setPassword(password);
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
            <SupplyIncreaseVestingStakingDialog
                open={dialogVestingOpen}
                setOpen={(isOpen) => setDialogVestingOpen(isOpen)}
            />
            <MigrationDialog
                open={dialogMigrationOpen}
                setOpen={(isOpen) => setDialogMigrationOpen(isOpen)}
            />
        </>
    );
}
