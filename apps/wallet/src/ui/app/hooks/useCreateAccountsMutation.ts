// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli, type AddedAccountsProperties } from '_src/shared/analytics/ampli';
import { useMutation } from '@tanstack/react-query';

import { useAccountsFormContext, AccountsFormType, type AccountsFormValues } from '_components';
import { useBackgroundClient } from './useBackgroundClient';
import { AccountType } from '_src/background/accounts/account';

import { useCreatePasskeyAccount } from './useCreatePasskeyAccount';

function validateAccountFormValues<T extends AccountsFormType>(
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
        values.type !== AccountsFormType.MnemonicSource &&
        values.type !== AccountsFormType.SeedSource &&
        !password
    ) {
        throw new Error('Missing password');
    }
    return true;
}

enum AmpliAccountType {
    Derived = 'Derived',
    ImportPrivateKey = 'Private Key',
    Passkey = 'Passkey',
    Ledger = 'Ledger',
    Keystone = 'Keystone',
}

export function useCreateAccountsMutation() {
    const backgroundClient = useBackgroundClient();
    const [accountsFormValuesRef, setAccountFormValues] = useAccountsFormContext();
    const { createPasskeyAccount } = useCreatePasskeyAccount();

    const CREATE_TYPE_TO_AMPLI_ACCOUNT: Record<
        AccountsFormType,
        AddedAccountsProperties['accountType']
    > = {
        [AccountsFormType.NewMnemonic]: AmpliAccountType.Derived,
        [AccountsFormType.ImportMnemonic]: AmpliAccountType.Derived,
        [AccountsFormType.ImportSeed]: AmpliAccountType.Derived,
        [AccountsFormType.MnemonicSource]: AmpliAccountType.Derived,
        [AccountsFormType.SeedSource]: AmpliAccountType.Derived,
        [AccountsFormType.ImportPrivateKey]: AmpliAccountType.ImportPrivateKey,
        [AccountsFormType.Passkey]: AmpliAccountType.Passkey,
        [AccountsFormType.ImportPasskey]: AmpliAccountType.Passkey,
        [AccountsFormType.ImportLedger]: AmpliAccountType.Ledger,
        [AccountsFormType.ImportKeystone]: AmpliAccountType.Keystone,
    };
    return useMutation({
        mutationKey: ['create accounts'],
        mutationFn: async ({ type, password }: { type: AccountsFormType; password?: string }) => {
            let createdAccounts;
            const accountsFormValues = accountsFormValuesRef.current;
            if (
                (type === AccountsFormType.NewMnemonic ||
                    type === AccountsFormType.ImportMnemonic) &&
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
                type === AccountsFormType.MnemonicSource &&
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
                type === AccountsFormType.ImportSeed &&
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
                type === AccountsFormType.SeedSource &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                if (password) {
                    await backgroundClient.unlockAccountSourceOrAccount({
                        password,
                        id: accountsFormValues.sourceID,
                    });
                }
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.SeedDerived,
                    sourceID: accountsFormValues.sourceID,
                });
            } else if (
                type === AccountsFormType.ImportPrivateKey &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.PrivateKeyDerived,
                    keyPair: accountsFormValues.keyPair,
                    password: password!,
                });
            } else if (
                type === AccountsFormType.Passkey &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                const { address, publicKey, providerOptions, credentialId } =
                    await createPasskeyAccount({
                        username: accountsFormValues.username,
                        authenticatorAttachment: accountsFormValues.authenticatorAttachment,
                    });

                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.PasskeyDerived,
                    address,
                    publicKey,
                    providerOptions,
                    credentialId,
                    password: password!,
                });
            } else if (
                type === AccountsFormType.ImportPasskey &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                const { address, publicKey, providerOptions, credentialId } =
                    await createPasskeyAccount({
                        isRestore: true,
                    });

                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.PasskeyDerived,
                    address,
                    publicKey,
                    providerOptions,
                    credentialId,
                    password: password!,
                });
            } else if (
                type === AccountsFormType.ImportLedger &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.LedgerDerived,
                    accounts: accountsFormValues.accounts,
                    password: password!,
                    mainPublicKey: accountsFormValues.mainPublicKey,
                });
            } else if (
                type === AccountsFormType.ImportKeystone &&
                validateAccountFormValues(type, accountsFormValues, password)
            ) {
                const sourceID = `keystone-${accountsFormValues.masterFingerprint}`;
                try {
                    await backgroundClient.createKeystoneAccountSource({
                        // validateAccountFormValues checks the password
                        password: password!,
                        masterFingerprint: accountsFormValues.masterFingerprint,
                    });
                } catch {
                    // Its fine to ignore if the account source already exists
                }

                await backgroundClient.unlockAccountSourceOrAccount({
                    password,
                    id: sourceID,
                });
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.KeystoneDerived,
                    accounts: accountsFormValues.accounts,
                    sourceID,
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
