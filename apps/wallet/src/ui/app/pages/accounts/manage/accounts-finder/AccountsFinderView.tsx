// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search24 } from '@iota/icons';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { AccountBalanceItem } from '_src/ui/app/components/accounts/AccountBalanceItem';
import { useAccountsFinder } from '_src/ui/app/hooks/useAccountsFinder';
import { useParams } from 'react-router-dom';
import { AllowedAccountSourceTypes } from '_src/ui/app/accounts-finder';
import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { getKey } from '_src/ui/app/helpers/accounts';
import { useMemo, useState } from 'react';
import { VerifyPasswordModal } from '_src/ui/app/components/accounts/VerifyPasswordModal';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useUnlockMutation } from '_src/ui/app/hooks/useUnlockMutation';
import { AccountType } from '_src/background/accounts/Account';
import { ConnectLedgerModal } from '_src/ui/app/components/ledger/ConnectLedgerModal';
import toast from 'react-hot-toast';
import { getLedgerConnectionErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useIotaLedgerClient } from '_src/ui/app/components/ledger/IotaLedgerClientProvider';
import { type AccountSourceSerializedUI } from '_src/background/account-sources/AccountSource';
import { type SourceStrategyToFind } from '_src/shared/messaging/messages/payloads/accounts-finder';

function getAccountSourceType(
    accountSource?: AccountSourceSerializedUI,
): AllowedAccountSourceTypes {
    if (accountSource) {
        return accountSource.type as unknown as AllowedAccountSourceTypes;
    } else {
        return AllowedAccountSourceTypes.LedgerDerived;
    }
}

export function AccountsFinderView(): JSX.Element {
    const { accountSourceId } = useParams();
    const { data: accountSources } = useAccountSources();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceId);
    const accountSourceType = getAccountSourceType(accountSource);
    const { data: accounts } = useAccounts();
    const persistedAccounts = accounts?.filter((acc) => getKey(acc) === accountSourceId);
    const [searched, setSearched] = useState(false);
    const [password, setPassword] = useState('');
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
        [password, accountSourceId],
    );
    const { find } = useAccountsFinder({
        accountSourceType,
        sourceStrategy,
    });
    const ledgerIotaClient = useIotaLedgerClient();
    const unlockAccountSourceMutation = useUnlockMutation();
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(false);

    async function findMore() {
        if (accountSourceId === AccountType.LedgerDerived && !ledgerIotaClient.iotaLedgerClient) {
            return setConnectLedgerModalOpen(true);
        }

        if (
            accountSource?.isLocked ||
            (accountSourceId === AccountType.LedgerDerived && !password)
        ) {
            return setPasswordModalVisible(true);
        }

        await find();
        setSearched(true);
    }

    return (
        <>
            <div className="flex h-full flex-1 flex-col justify-between">
                <div className="flex h-96 flex-col gap-4 overflow-y-auto">
                    {persistedAccounts?.map((account) => {
                        return <AccountBalanceItem key={account.id} account={account} />;
                    })}
                </div>
                <div className="flex flex-col gap-2">
                    <Button
                        variant="outline"
                        size="tall"
                        text={searched ? 'Search again' : 'Search'}
                        after={<Search24 />}
                        onClick={findMore}
                    />

                    <div className="flex flex-row gap-2">
                        <Button variant="outline" size="tall" text="Skip" />
                        <Button variant="outline" size="tall" text="Continue" />
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
