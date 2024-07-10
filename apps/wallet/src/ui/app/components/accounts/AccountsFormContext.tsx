// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createContext,
    useCallback,
    useContext,
    useMemo,
    useRef,
    type MutableRefObject,
    type ReactNode,
} from 'react';

export enum AccountsFormType {
    NewMnemonic = 'new-mnemonic',
    ImportMnemonic = 'import-mnemonic',
    ImportSeed = 'import-seed',
    ImportPrivateKey = 'import-private-key',
    ImportLedger = 'import-ledger',
    MnemonicSource = 'mnemonic-source',
    SeedSource = 'seed-source',
}

export type AccountsFormValues =
    | { type: AccountsFormType.NewMnemonic }
    | { type: AccountsFormType.ImportMnemonic; entropy: string }
    | { type: AccountsFormType.ImportSeed; seed: string }
    | { type: AccountsFormType.MnemonicSource; sourceID: string }
    | { type: AccountsFormType.SeedSource; sourceID: string }
    | { type: AccountsFormType.ImportPrivateKey; keyPair: string }
    | {
          type: AccountsFormType.ImportLedger;
          accounts: { publicKey: string; derivationPath: string; address: string }[];
      }
    | null;

type AccountsFormContextType = [
    MutableRefObject<AccountsFormValues>,
    (values: AccountsFormValues) => void,
];

const AccountsFormContext = createContext<AccountsFormContextType | null>(null);

interface AccountsFormProviderProps {
    children: ReactNode;
}

export function AccountsFormProvider({ children }: AccountsFormProviderProps) {
    const valuesRef = useRef<AccountsFormValues>(null);
    const setter = useCallback((values: AccountsFormValues) => {
        valuesRef.current = values;
    }, []);
    const value = useMemo(() => [valuesRef, setter] as AccountsFormContextType, [setter]);
    return <AccountsFormContext.Provider value={value}>{children}</AccountsFormContext.Provider>;
}

// a simple hook that allows form values to be shared between forms when setting up an account
// for the first time, or when importing an existing account.
export function useAccountsFormContext() {
    const context = useContext(AccountsFormContext);
    if (!context) {
        throw new Error('useAccountsFormContext must be used within the AccountsFormProvider');
    }
    return context;
}
