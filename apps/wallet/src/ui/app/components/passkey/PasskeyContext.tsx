// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useMemo } from 'react';
import {
    BrowserPasskeyProvider,
    findCommonPublicKey,
    PasskeyKeypair,
} from '@iota/iota-sdk/keypairs/passkey';

export interface PasskeyContextType {
    requestSignature: (data: Uint8Array, rpId: string, rpName: string) => Promise<string>;
}

export const PasskeyContext = createContext<PasskeyContextType | undefined>(undefined);

export function PasskeyProvider({ children }: { children: React.ReactNode }) {
    const context = useMemo(() => {
        return {
            requestSignature: (data: Uint8Array, rpId: string, rpName: string) =>
                new Promise<string>((resolve, reject) => {
                    const executeAsync = async () => {
                        try {
                            const provider = new BrowserPasskeyProvider(rpName, {
                                rp: {
                                    name: rpId,
                                    id: rpName,
                                },
                            });
                            // Generate two test messages to identify the correct public key
                            const testMessage1 = new TextEncoder().encode('IOTA Auth Message 1');
                            const possiblePks1 = await PasskeyKeypair.signAndRecover(
                                provider,
                                testMessage1,
                            );

                            const testMessage2 = new TextEncoder().encode('IOTA Auth Message 2');
                            const possiblePks2 = await PasskeyKeypair.signAndRecover(
                                provider,
                                testMessage2,
                            );

                            // Find the common public key
                            const commonPk = findCommonPublicKey(possiblePks1, possiblePks2);

                            // Create the keypair with the identified public key
                            const keypair = new PasskeyKeypair(commonPk.toRawBytes(), provider);

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
