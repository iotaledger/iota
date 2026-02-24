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

enum AmpliAccountOrigin {
    New = 'new',
    Import = 'import',
    Derived = 'derived',
}

export const ACCOUNT_FORM_TYPE_TO_AMPLI_ACCOUNT_TYPE: Record<
    AccountsFormType,
    AccountsAddedProperties['accountType']
> = {
    [AccountsFormType.NewMnemonic]: AmpliAccountType.Mnemonic,
    [AccountsFormType.ImportMnemonic]: AmpliAccountType.Mnemonic,
    [AccountsFormType.ImportSeed]: AmpliAccountType.Seed,
    [AccountsFormType.MnemonicSource]: AmpliAccountType.Mnemonic,
    [AccountsFormType.SeedSource]: AmpliAccountType.Seed,
    [AccountsFormType.ImportPrivateKey]: AmpliAccountType.PrivateKey,
    [AccountsFormType.Passkey]: AmpliAccountType.Passkey,
    [AccountsFormType.ImportPasskey]: AmpliAccountType.Passkey,
    [AccountsFormType.ImportLedger]: AmpliAccountType.Ledger,
    [AccountsFormType.ImportKeystone]: AmpliAccountType.Keystone,
};

export const ACCOUNT_FORM_TYPE_TO_ACCOUNT_ORIGIN: Record<AccountsFormType, AmpliAccountOrigin> = {
    [AccountsFormType.NewMnemonic]: AmpliAccountOrigin.New,
    [AccountsFormType.ImportMnemonic]: AmpliAccountOrigin.Import,
    [AccountsFormType.ImportSeed]: AmpliAccountOrigin.Import,
    [AccountsFormType.MnemonicSource]: AmpliAccountOrigin.Derived,
    [AccountsFormType.SeedSource]: AmpliAccountOrigin.Derived,
    [AccountsFormType.ImportPrivateKey]: AmpliAccountOrigin.Import,
    [AccountsFormType.Passkey]: AmpliAccountOrigin.New,
    [AccountsFormType.ImportPasskey]: AmpliAccountOrigin.Import,
    [AccountsFormType.ImportLedger]: AmpliAccountOrigin.Import,
    [AccountsFormType.ImportKeystone]: AmpliAccountOrigin.Import,
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
