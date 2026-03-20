// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import {
    createContext,
    useCallback,
    useContext,
    useEffect,
    useMemo,
    useRef,
    type MutableRefObject,
    type ReactNode,
} from 'react';
import { useSearchParams } from 'react-router-dom';
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
    MutableRefObject<AmpliSourceFlow>,
    (sourceFlow: AmpliSourceFlow) => void,
];

const AccountsFormContext = createContext<AccountsFormContextType | null>(null);

interface AccountsFormProviderProps {
    children: ReactNode;
}

const SOURCE_FLOW_SESSION_KEY = 'ampli_source_flow';

export function AccountsFormProvider({ children }: AccountsFormProviderProps) {
    const valuesRef = useRef<AccountsFormValues>(null);
    const setter = useCallback((values: AccountsFormValues) => {
        valuesRef.current = values;
    }, []);
    const sourceFlowRef = useRef<AmpliSourceFlow>(
        (sessionStorage.getItem(SOURCE_FLOW_SESSION_KEY) as AmpliSourceFlow) ??
            AmpliSourceFlow.Unknown,
    );
    const sourceFlowSetter = useCallback((sourceFlow: AmpliSourceFlow) => {
        sourceFlowRef.current = sourceFlow;
        sessionStorage.setItem(SOURCE_FLOW_SESSION_KEY, sourceFlow);
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
    const resetSourceFlow = useCallback(() => {
        sourceFlowRef.current = AmpliSourceFlow.Unknown;
        sessionStorage.removeItem(SOURCE_FLOW_SESSION_KEY);
    }, [sourceFlowRef]);
    return { sourceFlowRef, setSourceFlow, resetSourceFlow };
}

/**
 * Bootstraps sourceFlow from the `sourceFlow` URL query param.
 * Use this in pages that can be opened in a new browser tab (e.g. Ledger, Keystone, Passkey
 * popup flows) so the analytics value is preserved across the tab boundary.
 */
export function useBootstrapSourceFlow() {
    const [searchParams] = useSearchParams();
    const { setSourceFlow } = useSourceFlow();
    const urlSourceFlow = searchParams.get('sourceFlow');
    useEffect(() => {
        if (urlSourceFlow) {
            setSourceFlow(urlSourceFlow as AmpliSourceFlow);
        }
    }, [urlSourceFlow, setSourceFlow]);
}
