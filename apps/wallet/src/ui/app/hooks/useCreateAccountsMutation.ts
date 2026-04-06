// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ampli, ACCOUNT_FORM_TYPE_TO_AMPLI, AmpliSourceFlow } from '_src/shared/analytics';
import { useMutation } from '@tanstack/react-query';
import {
    useAccountsFormContext,
    AccountsFormType,
    type AccountsFormValues,
    useSourceFlow,
} from '_components';
import { useBackgroundClient } from './useBackgroundClient';
import { AccountType } from '_src/background/accounts/account';
import { useCreatePasskeyAccount } from './useCreatePasskeyAccount';
import { useAccounts } from './useAccounts';
import { isFirstAccount } from '_src/ui/app/helpers';

function validateAccountFormValues(
    createType: AccountsFormType,
    values: AccountsFormValues | null,
): Exclude<AccountsFormValues, null> {
    if (!values) {
        throw new Error('Missing account data values');
    }
    if (values.type !== createType) {
        throw new Error('Account data values type mismatch');
    }
    return values;
}

function ensurePassword(password: string | undefined): string {
    if (!password) {
        throw new Error('Missing password');
    }
    return password;
}

export function useCreateAccountsMutation() {
    const backgroundClient = useBackgroundClient();
    const [accountsFormValuesRef, setAccountFormValues, sourceFlowRef] = useAccountsFormContext();
    const { createPasskeyAccount } = useCreatePasskeyAccount();
    const { data: accounts } = useAccounts();
    const { resetSourceFlow } = useSourceFlow();

    return useMutation({
        mutationKey: ['create accounts'],
        onSuccess: () => {
            resetSourceFlow();
        },
        mutationFn: async ({ type, password }: { type: AccountsFormType; password?: string }) => {
            let createdAccounts;
            const accountsFormValues = accountsFormValuesRef.current;
            const sourceFlow = sourceFlowRef.current || AmpliSourceFlow.Unknown;
            const ampliData = ACCOUNT_FORM_TYPE_TO_AMPLI[type];

            // Validate form values are present and match the requested type
            const values = validateAccountFormValues(type, accountsFormValues);

            if (values.type === AccountsFormType.MnemonicSource) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.MnemonicDerived,
                    sourceID: values.sourceID,
                });
            } else if (values.type === AccountsFormType.SeedSource) {
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.SeedDerived,
                    sourceID: values.sourceID,
                });
            } else if (
                values.type === AccountsFormType.NewMnemonic ||
                values.type === AccountsFormType.ImportMnemonic
            ) {
                const validatedPassword = ensurePassword(password);
                const accountSource = await backgroundClient.createMnemonicAccountSource({
                    password: validatedPassword,
                    entropy: 'entropy' in values ? values.entropy : undefined,
                });

                await backgroundClient.unlockAccountSource({
                    id: accountSource.id,
                    password: validatedPassword,
                });

                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.MnemonicDerived,
                    sourceID: accountSource.id,
                });
            } else if (values.type === AccountsFormType.ImportSeed) {
                const validatedPassword = ensurePassword(password);
                const accountSource = await backgroundClient.createSeedAccountSource({
                    password: validatedPassword,
                    seed: values.seed,
                });

                await backgroundClient.unlockAccountSource({
                    id: accountSource.id,
                    password: validatedPassword,
                });

                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.SeedDerived,
                    sourceID: accountSource.id,
                });
            } else if (values.type === AccountsFormType.ImportPrivateKey) {
                const validatedPassword = ensurePassword(password);
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.PrivateKeyDerived,
                    keyPair: values.keyPair,
                    password: validatedPassword,
                });
            } else if (values.type === AccountsFormType.Passkey) {
                const validatedPassword = ensurePassword(password);
                const { address, publicKey, providerOptions, credentialId } =
                    await createPasskeyAccount({
                        username: values.username,
                        authenticatorAttachment: values.authenticatorAttachment,
                    });

                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.PasskeyDerived,
                    address,
                    publicKey,
                    providerOptions,
                    credentialId,
                    password: validatedPassword,
                });
            } else if (values.type === AccountsFormType.ImportPasskey) {
                const validatedPassword = ensurePassword(password);
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
                    password: validatedPassword,
                });
            } else if (values.type === AccountsFormType.ImportLedger) {
                const validatedPassword = ensurePassword(password);
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.LedgerDerived,
                    accounts: values.accounts,
                    password: validatedPassword,
                    mainPublicKey: values.mainPublicKey,
                });
            } else if (values.type === AccountsFormType.ImportKeystone) {
                const validatedPassword = ensurePassword(password);
                const sourceID = `keystone-${values.masterFingerprint}`;
                try {
                    await backgroundClient.createKeystoneAccountSource({
                        password: validatedPassword,
                        masterFingerprint: values.masterFingerprint,
                    });
                } catch {
                    // It's fine to ignore if the account source already exists
                }

                await backgroundClient.unlockAccountSource({
                    password: validatedPassword,
                    id: sourceID,
                });
                createdAccounts = await backgroundClient.createAccounts({
                    type: AccountType.KeystoneDerived,
                    accounts: values.accounts,
                    sourceID,
                });
            } else {
                throw new Error(`Create accounts with type ${type} is not implemented yet`);
            }

            ampli.addedAccounts({
                ...ampliData,
                sourceFlow,
                numberOfAccounts: createdAccounts.length,
                isFirstAccount: isFirstAccount(accounts),
            });
            setAccountFormValues(null);
            if (password) {
                await backgroundClient.unlockAllAccountsAndSources({ password });
            }
            const selectedAccount = createdAccounts[0];
            if (selectedAccount?.id) {
                await backgroundClient.selectAccount(selectedAccount?.id);
            }
            return createdAccounts;
        },
    });
}
