// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { type AccountsAddedProperties } from '_src/shared/analytics/ampli';
import { AccountType } from '_src/background/accounts/account';
import { AccountsFormType } from '_components/accounts';

enum AmpliAccountType {
    PrivateKey = 'Private Key',
    Passkey = 'Passkey',
    Ledger = 'Ledger',
    Keystone = 'Keystone',
    Mnemonic = 'Mnemonic',
    Seed = 'Seed',
}

export enum AmpliAccountOrigin {
    New = 'new',
    Import = 'import',
    Derived = 'derived',
}

export const ACCOUNT_FORM_TYPE_TO_AMPLI: Record<
    AccountsFormType,
    {
        accountType: AccountsAddedProperties['accountType'];
        accountOrigin: AmpliAccountOrigin;
        sourceFlow: AccountsAddedProperties['sourceFlow'];
    }
> = {
    [AccountsFormType.NewMnemonic]: {
        accountType: AmpliAccountType.Mnemonic,
        accountOrigin: AmpliAccountOrigin.New,
        sourceFlow: 'New Mnemonic',
    },
    [AccountsFormType.ImportMnemonic]: {
        accountType: AmpliAccountType.Mnemonic,
        accountOrigin: AmpliAccountOrigin.Import,
        sourceFlow: 'Import Mnemonic',
    },
    [AccountsFormType.ImportSeed]: {
        accountType: AmpliAccountType.Seed,
        accountOrigin: AmpliAccountOrigin.Import,
        sourceFlow: 'Import Seed',
    },
    [AccountsFormType.MnemonicSource]: {
        accountType: AmpliAccountType.Mnemonic,
        accountOrigin: AmpliAccountOrigin.Derived,
        sourceFlow: 'Derived Mnemonic',
    },
    [AccountsFormType.SeedSource]: {
        accountType: AmpliAccountType.Seed,
        accountOrigin: AmpliAccountOrigin.Derived,
        sourceFlow: 'Derived Seed',
    },
    [AccountsFormType.ImportPrivateKey]: {
        accountType: AmpliAccountType.PrivateKey,
        accountOrigin: AmpliAccountOrigin.Import,
        sourceFlow: 'Import Private Key',
    },
    [AccountsFormType.Passkey]: {
        accountType: AmpliAccountType.Passkey,
        accountOrigin: AmpliAccountOrigin.New,
        sourceFlow: 'New Passkey',
    },
    [AccountsFormType.ImportPasskey]: {
        accountType: AmpliAccountType.Passkey,
        accountOrigin: AmpliAccountOrigin.Import,
        sourceFlow: 'Import Passkey',
    },
    [AccountsFormType.ImportLedger]: {
        accountType: AmpliAccountType.Ledger,
        accountOrigin: AmpliAccountOrigin.Import,
        sourceFlow: 'Import Ledger',
    },
    [AccountsFormType.ImportKeystone]: {
        accountType: AmpliAccountType.Keystone,
        accountOrigin: AmpliAccountOrigin.Import,
        sourceFlow: 'Import Keystone',
    },
};

export const ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE: Record<
    AccountType,
    AccountsAddedProperties['accountType']
> = {
    [AccountType.MnemonicDerived]: AmpliAccountType.Mnemonic,
    [AccountType.SeedDerived]: AmpliAccountType.Seed,
    [AccountType.PrivateKeyDerived]: AmpliAccountType.PrivateKey,
    [AccountType.PasskeyDerived]: AmpliAccountType.Passkey,
    [AccountType.LedgerDerived]: AmpliAccountType.Ledger,
    [AccountType.KeystoneDerived]: AmpliAccountType.Keystone,
};
