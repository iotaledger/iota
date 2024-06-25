// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { AccountType } from '_src/background/accounts/Account';
import {
    createContext,
    useCallback,
    useContext,
    useMemo,
    useRef,
    type MutableRefObject,
    type ReactNode,
} from 'react';

export enum CreateAccountType {
    NewMnemonic = 'new-mnemonic',
    ImportMnemonic = 'import-mnemonic',
    ImportSeed = 'import-seed',
}

export type AccountsFormValues =
    | { type: CreateAccountType.NewMnemonic }
    | { type: CreateAccountType.ImportMnemonic; entropy: string }
    | { type: CreateAccountType.ImportSeed; seed: string }
    | { type: AccountType.MnemonicDerived; sourceID: string }
    | { type: AccountType.SeedDerived; sourceID: string }
    | { type: AccountType.Imported; keyPair: string }
    | {
          type: AccountType.Ledger;
          accounts: { publicKey: string; derivationPath: string; address: string }[];
      }
    | null;

type AccountsFormContextType = [
    MutableRefObject<AccountsFormValues>,
    (values: AccountsFormValues) => void,
];

const AccountsFormContext = createContext<AccountsFormContextType | null>(null);

export const AccountsFormProvider = ({ children }: { children: ReactNode }) => {
    const valuesRef = useRef<AccountsFormValues>(null);
    const setter = useCallback((values: AccountsFormValues) => {
        valuesRef.current = values;
    }, []);
    const value = useMemo(() => [valuesRef, setter] as AccountsFormContextType, [setter]);
    return <AccountsFormContext.Provider value={value}>{children}</AccountsFormContext.Provider>;
};

// a simple hook that allows form values to be shared between forms when setting up an account
// for the first time, or when importing an existing account.
export const useAccountsFormContext = () => {
    const context = useContext(AccountsFormContext);
    if (!context) {
        throw new Error('useAccountsFormContext must be used within the AccountsFormProvider');
    }
    return context;
};
