// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli, type AddedAccountsProperties } from '_src/shared/analytics/ampli';
import { useMutation } from '@tanstack/react-query';

import {
    CreateAccountType,
    useAccountsFormContext,
    type AccountsFormValues,
} from '../components/accounts/AccountsFormContext';
import { useBackgroundClient } from './useBackgroundClient';
import { AccountType } from '_src/background/accounts/Account';

export type CreateType = NonNullable<AccountsFormValues>['type'];

function validateAccountFormValues<T extends CreateType>(
    createType: T,
    values: AccountsFormValues,
    password?: string,
): values is Extract<AccountsFormValues, { type: T }> {
    if (!values) {
        throw new Error('Missing account data values');
    }
    if (values.type !== createType) {
        throw new Error('Account data values type mismatch');
    }
    if (
        values.type !== AccountType.MnemonicDerived &&
        values.type !== AccountType.SeedDerived &&
        !password
    ) {
        throw new Error('Missing password');
    }
    return true;
}
enum AmpliAccountType {
    Derived = 'Derived',
    Imported = 'Imported',
    Ledger = 'Ledger',
}
const CREATE_TYPE_TO_AMPLI_ACCOUNT: Record<CreateType, AddedAccountsProperties['accountType']> = {
    [CreateAccountType.NewMnemonic]: AmpliAccountType.Derived,
    [CreateAccountType.ImportMnemonic]: AmpliAccountType.Derived,
    [CreateAccountType.ImportSeed]: AmpliAccountType.Derived,
    [AccountType.MnemonicDerived]: AmpliAccountType.Derived,
    [AccountType.SeedDerived]: AmpliAccountType.Derived,
    [AccountType.Imported]: AmpliAccountType.Imported,
    [AccountType.Ledger]: AmpliAccountType.Ledger,
};

export function useCreateAccountsMutation() {
    const backgroundClient = useBackgroundClient();
    const [accountsFormValuesRef, setAccountFormValues] = useAccountsFormContext();
    return useMutation({
        mutationKey: ['create accounts'],
        mutationFn: async ({ type, password }: { type: CreateType; password?: string }) => {
            let createdAccounts;
            const accountsFormValues = accountsFormValuesRef.current;
            if (
                (type === CreateAccountType.NewMnemonic ||
                    type === CreateAccountType.ImportMnemonic) &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                const accountSource = await backgroundClient.createMnemonicAccountSource({
                    // validateAccountFormValues checks the password
                    password: password!,
                    entropy:
                        'entropy' in accountsFormValues ? accountsFormValues.entropy : undefined,
                });
                await backgroundClient.unlockAccountSourceOrAccount({
                    password,
                    id: accountSource.id,
                });
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.MnemonicDerived,
                    sourceID: accountSource.id,
                });
            } else if (
                type === AccountType.MnemonicDerived &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                if (password) {
                    await backgroundClient.unlockAccountSourceOrAccount({
                        password,
                        id: accountsFormValues.sourceID,
                    });
                }
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.MnemonicDerived,
                    sourceID: accountsFormValues.sourceID,
                });
            } else if (
                type === CreateAccountType.ImportSeed &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                const accountSource = await backgroundClient.createSeedAccountSource({
                    // validateAccountFormValues checks the password
                    password: password!,
                    seed: accountsFormValues.seed,
                });
                await backgroundClient.unlockAccountSourceOrAccount({
                    password,
                    id: accountSource.id,
                });
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.SeedDerived,
                    sourceID: accountSource.id,
                });
            } else if (
                type === AccountType.SeedDerived &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.SeedDerived,
                    sourceID: accountsFormValues.sourceID,
                });
            } else if (
                type === AccountType.Imported &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.Imported,
                    keyPair: accountsFormValues.keyPair,
                    password: password!,
                });
            } else if (
                type === AccountType.Ledger &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.Ledger,
                    accounts: accountsFormValues.accounts,
                    password: password!,
                });
            } else {
                throw new Error(`Create accounts with type ${type} is not implemented yet`);
            }
            for (const aCreatedAccount of createdAccounts) {
                await backgroundClient.unlockAccountSourceOrAccount({
                    id: aCreatedAccount.id,
                    password,
                });
            }
            ampli.addedAccounts({
                accountType: CREATE_TYPE_TO_AMPLI_ACCOUNT[type],
                numberOfAccounts: createdAccounts.length,
            });
            setAccountFormValues(null);
            const selectedAccount = createdAccounts[0];
            if (selectedAccount?.id) {
                await backgroundClient.selectAccount(selectedAccount?.id);
            }
            return createdAccounts;
        },
    });
}
