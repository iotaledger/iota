// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Search24 } from '@iota/icons';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { AccountBalanceItem } from '_src/ui/app/components/accounts/AccountBalanceItem';
import { useAccountsFinder } from '_src/ui/app/hooks/useAccountsFinder';
import { useParams } from 'react-router-dom';
import { useActiveAccount } from '_app/hooks/useActiveAccount';
import { type AllowedAccountTypes } from '_src/background/accounts-finder';
import { useAccounts } from '_src/ui/app/hooks/useAccounts';
import { getKey } from '_src/ui/app/helpers/accounts';
import { useState, useMemo } from 'react';
import { VerifyPasswordModal } from '_src/ui/app/components/accounts/VerifyPasswordModal';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useUnlockMutation } from '_src/ui/app/hooks/useUnlockMutation';
import LoadingIndicator from '_components/loading/LoadingIndicator';

export function AccountsFinderView(): JSX.Element {
    const { accountSourceId } = useParams();
    const { data: accounts } = useAccounts();
    const persistedAccounts = accounts?.filter((acc) => getKey(acc) === accountSourceId);
    const currentAccount = useActiveAccount();
    const [searched, setSearched] = useState(false);

    const { search, reset } = useAccountsFinder({
        accountType: currentAccount?.type as AllowedAccountTypes,
        sourceID: accountSourceId || '',
    });
    const { data: accountSources } = useAccountSources();
    const unlockAccountSourceMutation = useUnlockMutation();
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const [isSearchProcessing, setIsSearchProcessing] = useState(false);
    const accountSource = accountSources?.find(({ id }) => id === accountSourceId);

    async function searchMore() {
        if (accountSource?.isLocked) {
            setPasswordModalVisible(true);
        } else {
            try {
                setSearched(true);
                setIsSearchProcessing(true);
                await search();
            } finally {
                setIsSearchProcessing(false);
            }
        }
    }

    const searchOptions = useMemo(() => {
        if (isSearchProcessing)
            return {
                text: '',
                icon: <LoadingIndicator />,
            };

        return { text: searched ? 'Search again' : 'Search', icon: <Search24 /> };
    }, [searched, isSearchProcessing]);

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
                        text={'Start again'}
                        onClick={reset}
                        disabled={isSearchProcessing}
                    />
                    <Button
                        variant="outline"
                        size="tall"
                        text={searchOptions.text}
                        after={searchOptions.icon}
                        onClick={searchMore}
                        disabled={isSearchProcessing}
                    />

                    <div className="flex flex-row gap-2">
                        <Button
                            variant="outline"
                            size="tall"
                            text="Skip"
                            disabled={isSearchProcessing}
                        />
                        <Button
                            variant="outline"
                            size="tall"
                            text="Continue"
                            disabled={isSearchProcessing}
                        />
                    </div>
                </div>
            </div>
            {isPasswordModalVisible && accountSourceId ? (
                <VerifyPasswordModal
                    open
                    onVerify={async (password) => {
                        await unlockAccountSourceMutation.mutateAsync({
                            id: accountSourceId,
                            password,
                        });
                        setPasswordModalVisible(false);
                        search();
                    }}
                    onClose={() => setPasswordModalVisible(false)}
                />
            ) : null}
        </>
    );
}
