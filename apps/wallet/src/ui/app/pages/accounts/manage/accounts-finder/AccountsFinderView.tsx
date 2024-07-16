// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search24 } from '@iota/icons';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { AccountBalanceItem } from '_src/ui/app/components/accounts/AccountBalanceItem';
import { useAccountsFinder } from '_src/ui/app/hooks/useAccountsFinder';
import { useNavigate, useParams } from 'react-router-dom';
import { useActiveAccount } from '_app/hooks/useActiveAccount';
import { type AllowedAccountTypes } from '_src/ui/app/accounts-finder';
import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { getKey } from '_src/ui/app/helpers/accounts';
import { useState } from 'react';
import { VerifyPasswordModal } from '_src/ui/app/components/accounts/VerifyPasswordModal';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useUnlockMutation } from '_src/ui/app/hooks/useUnlockMutation';
import { AccountType } from '_src/background/accounts/Account';
import { ConnectLedgerModal } from '_src/ui/app/components/ledger/ConnectLedgerModal';
import toast from 'react-hot-toast';
import { getLedgerConnectionErrorMessage } from '_src/ui/app/helpers/errorMessages';
import { useIotaLedgerClient } from '_src/ui/app/components/ledger/IotaLedgerClientProvider';

export function AccountsFinderView(): JSX.Element {
    const { accountSourceId } = useParams();
    const { data: accounts } = useAccounts();
    const persistedAccounts = accounts?.filter((acc) => getKey(acc) === accountSourceId);
    const currentAccount = useActiveAccount();
    const [searched, setSearched] = useState(false);
    const [password, setPassword] = useState('');
    const { find } = useAccountsFinder({
        accountType: currentAccount?.type as AllowedAccountTypes,
        sourceStrategy:
            accountSourceId == AccountType.LedgerDerived
                ? {
                      type: 'ledger',
                      password,
                  }
                : {
                      type: 'software',
                      sourceID: accountSourceId!,
                  },
    });
    const ledgerIotaClinet = useIotaLedgerClient();
    const { data: accountSources } = useAccountSources();
    const unlockAccountSourceMutation = useUnlockMutation();
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const [isConnectLedgerModalOpen, setConnectLedgerModalOpen] = useState(false);

    const accountSource = accountSources?.find(({ id }) => id === accountSourceId);

    function findMore() {

        if (accountSourceId === AccountType.LedgerDerived && !ledgerIotaClinet){
            return setConnectLedgerModalOpen(true);
        }
        
        if (accountSource?.isLocked || accountSourceId === AccountType.LedgerDerived) {
            return setPasswordModalVisible(true);
        } 

        setSearched(true);
        find();
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
                        if (accountSourceId) {
                            // unlock software account sources
                            await unlockAccountSourceMutation.mutateAsync({
                                id: accountSourceId,
                                password,
                            });
                        } else {
                            // for ledger
                            setPassword(password);
                        }

                        setPasswordModalVisible(false);
                        find();
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
                    }}
                />
            )}
        </>
    );
}
