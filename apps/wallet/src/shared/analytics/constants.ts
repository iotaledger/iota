// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { type AddedAccountsProperties } from '_src/shared/analytics/ampli';
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

export enum AmpliSourceFlow {
    Onboarding = 'Onboarding',
    ManageAccounts = 'Manage Accounts',
    BalanceFinder = 'Balance Finder',
    Unknown = 'Unknown',
}

export enum AmpliAccountOrigin {
    New = 'new',
    Import = 'import',
    Derived = 'derived',
}

export const ACCOUNT_FORM_TYPE_TO_AMPLI: Record<
    AccountsFormType,
    {
        accountType: AddedAccountsProperties['accountType'];
        accountOrigin: AmpliAccountOrigin;
    }
> = {
    [AccountsFormType.NewMnemonic]: {
        accountType: AmpliAccountType.Mnemonic,
        accountOrigin: AmpliAccountOrigin.New,
    },
    [AccountsFormType.ImportMnemonic]: {
        accountType: AmpliAccountType.Mnemonic,
        accountOrigin: AmpliAccountOrigin.Import,
    },
    [AccountsFormType.ImportSeed]: {
        accountType: AmpliAccountType.Seed,
        accountOrigin: AmpliAccountOrigin.Import,
    },
    [AccountsFormType.MnemonicSource]: {
        accountType: AmpliAccountType.Mnemonic,
        accountOrigin: AmpliAccountOrigin.Derived,
    },
    [AccountsFormType.SeedSource]: {
        accountType: AmpliAccountType.Seed,
        accountOrigin: AmpliAccountOrigin.Derived,
    },
    [AccountsFormType.ImportPrivateKey]: {
        accountType: AmpliAccountType.PrivateKey,
        accountOrigin: AmpliAccountOrigin.Import,
    },
    [AccountsFormType.Passkey]: {
        accountType: AmpliAccountType.Passkey,
        accountOrigin: AmpliAccountOrigin.New,
    },
    [AccountsFormType.ImportPasskey]: {
        accountType: AmpliAccountType.Passkey,
        accountOrigin: AmpliAccountOrigin.Import,
    },
    [AccountsFormType.ImportLedger]: {
        accountType: AmpliAccountType.Ledger,
        accountOrigin: AmpliAccountOrigin.Import,
    },
    [AccountsFormType.ImportKeystone]: {
        accountType: AmpliAccountType.Keystone,
        accountOrigin: AmpliAccountOrigin.Import,
    },
};

export const ACCOUNT_TYPE_TO_AMPLI_ACCOUNT_TYPE: Record<
    AccountType,
    AddedAccountsProperties['accountType']
> = {
    [AccountType.MnemonicDerived]: AmpliAccountType.Mnemonic,
    [AccountType.SeedDerived]: AmpliAccountType.Seed,
    [AccountType.PrivateKeyDerived]: AmpliAccountType.PrivateKey,
    [AccountType.PasskeyDerived]: AmpliAccountType.Passkey,
    [AccountType.LedgerDerived]: AmpliAccountType.Ledger,
    [AccountType.KeystoneDerived]: AmpliAccountType.Keystone,
};
