// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import SpeculosHttpTransport from './SpeculosHttpTransport';
import IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';

import {
    convertErrorToLedgerConnectionFailedError,
    LedgerDeviceNotFoundError,
    LedgerNoTransportMechanismError,
} from './ledgerErrors';

type IotaLedgerClientProviderProps = {
    children: React.ReactNode;
};

type IotaLedgerClientContextValue = {
    iotaLedgerClient: IotaLedgerClient | undefined;
    connectToLedger: (requestPermissionsFirst?: boolean) => Promise<IotaLedgerClient>;
};

const IotaLedgerClientContext = createContext<IotaLedgerClientContextValue | undefined>(undefined);

export function IotaLedgerClientProvider({ children }: IotaLedgerClientProviderProps) {
    const [iotaLedgerClient, setIotaLedgerClient] = useState<IotaLedgerClient>();
    const resetIotaLedgerClient = useCallback(async () => {
        await iotaLedgerClient?.transport.close();
        setIotaLedgerClient(undefined);
    }, [iotaLedgerClient]);

    useEffect(() => {
        // NOTE: The disconnect event is fired when someone physically disconnects
        // their Ledger device in addition to when user's exit out of an application
        iotaLedgerClient?.transport.on('disconnect', resetIotaLedgerClient);
        return () => {
            iotaLedgerClient?.transport.off('disconnect', resetIotaLedgerClient);
        };
    }, [resetIotaLedgerClient, iotaLedgerClient?.transport]);

    const connectToLedger = useCallback(
        async (requestPermissionsFirst = false) => {
            // If we've already connected to a Ledger device, we need
            // to close the connection before we try to re-connect
            if (requestPermissionsFirst) {
                await requestLedgerConnection()
            }
            await resetIotaLedgerClient();
            const ledgerTransport = await openLedgerConnection();
            const ledgerClient = new IotaLedgerClient(ledgerTransport);
            setIotaLedgerClient(ledgerClient);
            return ledgerClient;
        },
        [resetIotaLedgerClient],
    );
    const contextValue: IotaLedgerClientContextValue = useMemo(() => {
        return {
            iotaLedgerClient,
            connectToLedger,
        };
    }, [connectToLedger, iotaLedgerClient]);

    return (
        <IotaLedgerClientContext.Provider value={contextValue}>
            {children}
        </IotaLedgerClientContext.Provider>
    );
}

async function requestLedgerConnection() {
    // const ledgerTransportClass = await getLedgerTransportClass();
    try {
        return await true;
    } catch (error) {
        throw convertErrorToLedgerConnectionFailedError(error);
    }
}

export function useIotaLedgerClient() {
    const iotaLedgerClientContext = useContext(IotaLedgerClientContext);
    if (!iotaLedgerClientContext) {
        throw new Error('useIotaLedgerClient must be used within IotaLedgerClientContext');
    }
    return iotaLedgerClientContext;
}

async function openLedgerConnection() {
    const ledgerTransportClass = await getLedgerTransportClass();
    let ledgerTransport: SpeculosHttpTransport | null | undefined;
    try {
        console.log('openingggg')
        ledgerTransport = await ledgerTransportClass.open({ baseURL: "", apiPort: '5005' });
    } catch (error) {
        console.error(error);
        throw convertErrorToLedgerConnectionFailedError(error);
    }
    if (!ledgerTransport) {
        throw new LedgerDeviceNotFoundError(
            "The user doesn't have a Ledger device connected to their machine",
        );
    }
    return ledgerTransport;
}

async function getLedgerTransportClass() {
    if (await SpeculosHttpTransport.isSupported()) {
        return SpeculosHttpTransport;
    }
    throw new LedgerNoTransportMechanismError(
        "There are no supported transport mechanisms to connect to the user's Ledger device",
    );
}
