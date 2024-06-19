// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { Text } from '_app/shared/text';
import { Heading } from '_app/shared/heading';
import { type SerializedUIAccount } from '_src/background/accounts/Account';
import * as ToggleGroup from '@radix-ui/react-toggle-group';
import { Button } from '_app/shared/ButtonUI';
import { AccountsFinderItem } from '_components/accounts/AccountsFinderItem';
import { useNavigate, useSearchParams } from 'react-router-dom';
import { SelectAllButton } from '_components/accounts/SelectAllButton';

function mockAccountFactory(id: string) {
    return {
        id: id,
        isLocked: false,
        address: 'mockAddress1',
        type: 'seed-derived' as const,
        publicKey: 'mockPublicKey1',
        lastUnlockedOn: 0,
        selected: false,
        nickname: 'mockNickname1',
        isPasswordUnlockable: true,
        isKeyPairExportable: true,
        formattedBalance: '1000000000000',
    };
}

const mockSerializedUIAccounts: (SerializedUIAccount & { formattedBalance: string })[] = [
    mockAccountFactory('mockID1'),
    mockAccountFactory('mockID2'),
    mockAccountFactory('mockID3'),
    mockAccountFactory('mockID4'),
    mockAccountFactory('mockID5'),
    mockAccountFactory('mockID6'),
    mockAccountFactory('mockID7'),
];

export function AccountsFinderPage2() {
    const [searchParams] = useSearchParams();
    const navigate = useNavigate();
    const successRedirect = searchParams.get('successRedirect') || '/tokens';
    const [selectedAccountIDs, setSelectedAccountIDs] = useState<string[]>([]);

    const onChange = (value: string[]) => {
        setSelectedAccountIDs(value);
    };

    const onNext = () => {
        navigate(successRedirect, { replace: true });
    };

    const onSearchMore = () => {
        // use search more logic here
    };

    return (
        <div className="flex h-full w-full flex-col items-center rounded-20 bg-iota-lightest px-6 py-10 shadow-wallet-content">
            <div className={''}>
                <Text variant="caption" color="steel-dark" weight="semibold">
                    Wallet Setup
                </Text>
                <div className="my-2.5 text-center">
                    <Heading variant="heading1" color="gray-90" as="h1" weight="bold">
                        Found accounts
                    </Heading>
                </div>
                {selectedAccountIDs.length ? (
                    <div className="my-2.5 text-center">
                        <Heading variant="heading6" color="gray-60" as="h6" weight="bold">
                            Selected accounts: {selectedAccountIDs.length}
                        </Heading>
                    </div>
                ) : null}
            </div>

            <div className="flex w-full flex-1 flex-col gap-3 overflow-auto [&>button]:border-none">
                <ToggleGroup.Root
                    value={selectedAccountIDs}
                    onValueChange={onChange}
                    type="multiple"
                    className="flex flex-col gap-3 overflow-y-auto"
                >
                    {mockSerializedUIAccounts.map((account) => (
                        <ToggleGroup.Item key={account.id} asChild value={account.id}>
                            <div>
                                <AccountsFinderItem
                                    disabled={account.isLocked}
                                    account={account}
                                    selected={selectedAccountIDs.includes(account.id)}
                                    formattedBalance={account.formattedBalance}
                                />
                            </div>
                        </ToggleGroup.Item>
                    ))}
                </ToggleGroup.Root>
            </div>
            <div className="mt-3 flex w-full flex-col gap-3">
                {mockSerializedUIAccounts.length > 1 ? (
                    <SelectAllButton
                        accountIds={mockSerializedUIAccounts.map((account) => account.id)}
                        selectedAccountIds={selectedAccountIDs}
                        onChange={onChange}
                    />
                ) : null}
                <div className="flex gap-2.5">
                    <Button
                        variant="outline"
                        size="tall"
                        text={'Search more'}
                        onClick={onSearchMore}
                    />
                    <Button variant="primary" size="tall" onClick={onNext} text={'Go to wallet'} />
                </div>
            </div>
        </div>
    );
}
