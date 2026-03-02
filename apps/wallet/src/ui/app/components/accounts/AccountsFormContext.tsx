// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
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
import { AmpliSourceFlow } from '_src/shared/analytics';

export enum AccountsFormType {
    NewMnemonic = 'new-mnemonic',
    ImportMnemonic = 'import-mnemonic',
    ImportSeed = 'import-seed',
    ImportPrivateKey = 'import-private-key',
    ImportLedger = 'import-ledger',
    Passkey = 'passkey',
    ImportPasskey = 'import-passkey',
    MnemonicSource = 'mnemonic-source',
    SeedSource = 'seed-source',
    ImportKeystone = 'import-keystone',
}

export type AccountsFormValues =
    | { type: AccountsFormType.NewMnemonic }
    | { type: AccountsFormType.ImportMnemonic; entropy: string }
    | { type: AccountsFormType.ImportSeed; seed: string }
    | { type: AccountsFormType.MnemonicSource; sourceID: string }
    | { type: AccountsFormType.SeedSource; sourceID: string }
    | { type: AccountsFormType.ImportPrivateKey; keyPair: string }
    | {
          type: AccountsFormType.Passkey;
          authenticatorAttachment: AuthenticatorAttachment;
          username: string;
      }
    | {
          type: AccountsFormType.ImportPasskey;
      }
    | {
          type: AccountsFormType.ImportLedger;
          mainPublicKey: string;
          accounts: { publicKey: string; derivationPath: string; address: string }[];
      }
    | {
          type: AccountsFormType.ImportKeystone;
          masterFingerprint: string;
          accounts: {
              publicKey: string;
              derivationPath: string;
              address: string;
          }[];
      }
    | null;

type AccountsFormContextType = [
    MutableRefObject<AccountsFormValues>,
    (values: AccountsFormValues) => void,
    MutableRefObject<string>,
    (sourceFlow: string) => void,
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
    const sourceFlowRef = useRef<string>(AmpliSourceFlow.Unknown);
    const sourceFlowSetter = useCallback((sourceFlow: string) => {
        sourceFlowRef.current = sourceFlow;
    }, []);
    const value = useMemo(
        () => [valuesRef, setter, sourceFlowRef, sourceFlowSetter] as AccountsFormContextType,
        [setter, sourceFlowSetter],
    );
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

/**
 * Hook to get and set the sourceFlow analytics value for the current account creation flow.
 * Set once at the start of the flow (WelcomePage / AddAccountPage), read wherever needed.
 */
export function useSourceFlow() {
    const context = useContext(AccountsFormContext);
    if (!context) {
        throw new Error('useSourceFlow must be used within the AccountsFormProvider');
    }
    const [, , sourceFlowRef, setSourceFlow] = context;
    return { sourceFlowRef, setSourceFlow };
}
