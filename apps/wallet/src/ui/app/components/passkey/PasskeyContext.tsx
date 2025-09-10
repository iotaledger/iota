// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { createContext, useContext, useState } from 'react';
import { findCommonPublicKey, PasskeyKeypair } from '@iota/iota-sdk/keypairs/passkey';
import { PASSKEY_PROVIDER } from './passkey-provider';

export interface PasskeyContextType {
    passkeyKeypair: PasskeyKeypair | null;
    isRegistering: boolean;
    isAuthenticating: boolean;
    registerPasskey: () => Promise<void>;
    authenticateWithPasskey: () => Promise<void>;
    signOut: () => void;
    error: string | null;
}

export const PasskeyContext = createContext<PasskeyContextType | undefined>(undefined);

export function PasskeyProvider({ children }: { children: React.ReactNode }) {
    const [passkeyKeypair, setPasskeyKeypair] = useState<PasskeyKeypair | null>(null);
    const [isRegistering, setIsRegistering] = useState(false);
    const [isAuthenticating, setIsAuthenticating] = useState(false);
    const [error, setError] = useState<string | null>(null);

    const registerPasskey = async () => {
        setIsRegistering(true);
        setError(null);
        try {
            const provider = PASSKEY_PROVIDER;

            const keypair = await PasskeyKeypair.getPasskeyInstance(provider);
            setPasskeyKeypair(keypair);

            // Store public key information in local storage for later identification
            localStorage.setItem(
                'walletPublicKey',
                JSON.stringify(Array.from(keypair.getPublicKey().toRawBytes())),
            );
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Failed to register passkey');
        } finally {
            setIsRegistering(false);
        }
    };

    const authenticateWithPasskey = async () => {
        setIsAuthenticating(true);
        setError(null);
        try {
            const provider = PASSKEY_PROVIDER;

            // Generate two test messages to identify the correct public key
            const testMessage1 = new TextEncoder().encode('IOTA Auth Message 1');
            const possiblePks1 = await PasskeyKeypair.signAndRecover(provider, testMessage1);

            const testMessage2 = new TextEncoder().encode('IOTA Auth Message 2');
            const possiblePks2 = await PasskeyKeypair.signAndRecover(provider, testMessage2);

            // Find the common public key
            const commonPk = findCommonPublicKey(possiblePks1, possiblePks2);

            // Create the keypair with the identified public key
            const keypair = new PasskeyKeypair(commonPk.toRawBytes(), provider);
            setPasskeyKeypair(keypair);
        } catch (e) {
            setError(e instanceof Error ? e.message : 'Failed to authenticate with passkey');
        } finally {
            setIsAuthenticating(false);
        }
    };

    const signOut = () => {
        setPasskeyKeypair(null);
    };

    return (
        <PasskeyContext.Provider
            value={{
                passkeyKeypair,
                isRegistering,
                isAuthenticating,
                registerPasskey,
                authenticateWithPasskey,
                signOut,
                error,
            }}
        >
            {children}
        </PasskeyContext.Provider>
    );
}

export const usePasskey = () => {
    const context = useContext(PasskeyContext);
    if (context === undefined) {
        throw new Error('usePasskey must be used within a PasskeyProvider');
    }
    return context;
};
