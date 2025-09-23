// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useMemo } from 'react';
import {
    type BrowserPasswordProviderOptions,
    PasskeyKeypair,
} from '@iota/iota-sdk/keypairs/passkey';
import { useRestoreWallet } from '../../hooks/useRestorePasskey';
import { fromBase64 } from '@iota/iota-sdk/utils';
import { createBrowserPasskeyProvider } from './passkey-provider';

export interface PasskeyContextType {
    requestSignature: (
        data: Uint8Array,
        providerOptions: BrowserPasswordProviderOptions,
        publicKey?: string,
    ) => Promise<string>;
}

export const PasskeyContext = createContext<PasskeyContextType | undefined>(undefined);

export function PasskeyProvider({ children }: { children: React.ReactNode }) {
    const { mutateAsync: restoreWallet } = useRestoreWallet();

    const context = useMemo(() => {
        return {
            requestSignature: (
                data: Uint8Array,
                providerOptions: BrowserPasswordProviderOptions,
                publicKey: string | undefined,
            ) =>
                new Promise<string>((resolve, reject) => {
                    const executeAsync = async () => {
                        try {
                            const provider = createBrowserPasskeyProvider({
                                options: providerOptions,
                            });

                            let keypair: PasskeyKeypair;
                            if (publicKey) {
                                const publicKeyBytes = fromBase64(publicKey);
                                keypair = new PasskeyKeypair(publicKeyBytes, provider);
                            } else {
                                keypair = await restoreWallet(provider);
                            }

                            const { signature } = await keypair.signTransaction(data);

                            resolve(signature);
                        } catch (error) {
                            reject(error);
                        }
                    };

                    executeAsync();
                }),
        };
    }, []);

    return <PasskeyContext.Provider value={context}>{children}</PasskeyContext.Provider>;
}

export const usePasskeyContext = () => {
    const context = useContext(PasskeyContext);
    if (context === undefined) {
        throw new Error('usePasskey must be used within a PasskeyProvider');
    }
    return context;
};
