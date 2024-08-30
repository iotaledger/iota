// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { AccountType, type SerializedUIAccount } from '_src/background/accounts/Account';
import {
    AccountIcon,
    AccountItem,
    AccountsFormType,
    useAccountsFormContext,
    VerifyPasswordModal,
} from '_components';
import { useAccountSources } from '_src/ui/app/hooks/useAccountSources';
import { useCreateAccountsMutation } from '_src/ui/app/hooks/useCreateAccountMutation';
import { Button } from '_src/ui/app/shared/ButtonUI';
import { Heading } from '_src/ui/app/shared/heading';
import { Text } from '_src/ui/app/shared/text';
import { ButtonOrLink } from '_src/ui/app/shared/utils/ButtonOrLink';
import { ArrowBgFill16, Plus12, Search16 } from '@iota/icons';
import * as CollapsiblePrimitive from '@radix-ui/react-collapsible';
import { useState } from 'react';
import { useNavigate } from 'react-router-dom';

const ACCOUNT_TYPE_TO_LABEL: Record<AccountType, string> = {
    [AccountType.MnemonicDerived]: 'Passphrase Derived',
    [AccountType.SeedDerived]: 'Seed Derived',
    [AccountType.PrivateKeyDerived]: 'Private Key',
    [AccountType.LedgerDerived]: 'Ledger',
};
const ACCOUNTS_WITH_ENABLED_BALANCE_FINDER: AccountType[] = [
    AccountType.MnemonicDerived,
    AccountType.SeedDerived,
    AccountType.LedgerDerived,
];

export function getGroupTitle(aGroupAccount: SerializedUIAccount) {
    return ACCOUNT_TYPE_TO_LABEL[aGroupAccount?.type] || '';
}

export function AccountGroup({
    accounts,
    type,
    accountSourceID,
}: {
    accounts: SerializedUIAccount[];
    type: AccountType;
    accountSourceID?: string;
}) {
    const navigate = useNavigate();
    const createAccountMutation = useCreateAccountsMutation();
    const isMnemonicDerivedGroup = type === AccountType.MnemonicDerived;
    const isSeedDerivedGroup = type === AccountType.SeedDerived;
    const [accountsFormValues, setAccountsFormValues] = useAccountsFormContext();
    const [isPasswordModalVisible, setPasswordModalVisible] = useState(false);
    const { data: accountSources } = useAccountSources();
    const accountSource = accountSources?.find(({ id }) => id === accountSourceID);
    return (
        <>
            <CollapsiblePrimitive.Root defaultOpen asChild>
                <div className="flex w-full flex-col gap-4">
                    <CollapsiblePrimitive.Trigger asChild>
                        <div className="group flex w-full flex-shrink-0 cursor-pointer items-center justify-center gap-2 [&>*]:select-none">
                            <ArrowBgFill16 className="text-hero-darkest/20 h-4 w-4 group-data-[state=open]:rotate-90" />
                            <Heading variant="heading5" weight="semibold" color="steel-darker">
                                {getGroupTitle(accounts[0])}
                            </Heading>
                            <div className="bg-gray-45 flex h-px flex-1 flex-shrink-0" />
                            {ACCOUNTS_WITH_ENABLED_BALANCE_FINDER.includes(type) ? (
                                <ButtonOrLink
                                    className="text-hero hover:text-hero-darkest flex cursor-pointer appearance-none items-center justify-center gap-0.5 border-0 bg-transparent uppercase outline-none"
                                    onClick={() => {
                                        navigate(
                                            `/accounts/manage/accounts-finder/${accountSourceID}`,
                                        );
                                    }}
                                >
                                    <Search16 />
                                </ButtonOrLink>
                            ) : null}
                            {(isMnemonicDerivedGroup || isSeedDerivedGroup) && accountSource ? (
                                <>
                                    <ButtonOrLink
                                        loading={createAccountMutation.isPending}
                                        onClick={async (e) => {
                                            // prevent the collapsible from closing when clicking the "new" button
                                            e.stopPropagation();
                                            const accountsFormType = isMnemonicDerivedGroup
                                                ? AccountsFormType.MnemonicSource
                                                : AccountsFormType.SeedSource;
                                            setAccountsFormValues({
                                                type: accountsFormType,
                                                sourceID: accountSource.id,
                                            });
                                            if (accountSource.isLocked) {
                                                setPasswordModalVisible(true);
                                            } else {
                                                createAccountMutation.mutate({
                                                    type: accountsFormType,
                                                });
                                            }
                                        }}
                                        className="text-hero hover:text-hero-darkest flex cursor-pointer appearance-none items-center justify-center gap-0.5 border-0 bg-transparent uppercase outline-none"
                                    >
                                        <Plus12 />
                                        <Text variant="bodySmall" weight="semibold">
                                            New
                                        </Text>
                                    </ButtonOrLink>
                                </>
                            ) : null}
                        </div>
                    </CollapsiblePrimitive.Trigger>
                    <CollapsiblePrimitive.CollapsibleContent asChild>
                        <div className="flex w-full flex-shrink-0 flex-col gap-3">
                            {accounts.map((account) => {
                                return (
                                    <AccountItem
                                        key={account.id}
                                        accountID={account.id}
                                        icon={<AccountIcon account={account} />}
                                    />
                                );
                            })}
                            {isMnemonicDerivedGroup && accountSource ? (
                                <Button
                                    variant="secondary"
                                    size="tall"
                                    text="Export Passphrase"
                                    to={`../export/passphrase/${accountSource.id}`}
                                />
                            ) : null}
                            {isSeedDerivedGroup && accountSource ? (
                                <Button
                                    variant="secondary"
                                    size="tall"
                                    text="Export Seed"
                                    to={`../export/seed/${accountSource.id}`}
                                />
                            ) : null}
                        </div>
                    </CollapsiblePrimitive.CollapsibleContent>
                </div>
            </CollapsiblePrimitive.Root>
            {isPasswordModalVisible ? (
                <VerifyPasswordModal
                    open
                    onVerify={async (password) => {
                        if (accountsFormValues.current) {
                            await createAccountMutation.mutateAsync({
                                type: accountsFormValues.current.type,
                                password,
                            });
                        }
                        setPasswordModalVisible(false);
                    }}
                    onClose={() => setPasswordModalVisible(false)}
                />
            ) : null}
        </>
    );
}
