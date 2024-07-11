// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import IotaLedgerClient from '@iota/ledgerjs-hw-app-iota';
import TransportWebHID from '@ledgerhq/hw-transport-webhid';
import TransportWebUSB from '@ledgerhq/hw-transport-webusb';
import SpeculosHttpTransport from '_src/ui/app/SpeculosHttpTransport';
import { createContext, useCallback, useContext, useEffect, useMemo, useState } from 'react';

import {
    convertErrorToLedgerConnectionFailedError,
    LedgerDeviceNotFoundError,
    LedgerNoTransportMechanismError,
} from './ledgerErrors';

interface IotaLedgerClientProviderProps {
    children: React.ReactNode;
}

interface IotaLedgerClientContextValue {
    iotaLedgerClient: IotaLedgerClient | undefined;
    connectToLedger: (requestPermissionsFirst?: boolean) => Promise<IotaLedgerClient>;
}

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
            let ledgerTransport: TransportWebHID | TransportWebUSB | SpeculosHttpTransport;
            // If we've already connected to a Ledger device, we need
            // to close the connection before we try to re-connect
            await resetIotaLedgerClient();

            if (await SpeculosHttpTransport.check()) {
                ledgerTransport = await SpeculosHttpTransport.open();
            } else {
                ledgerTransport = requestPermissionsFirst
                    ? await requestLedgerConnection()
                    : await openLedgerConnection();
            }

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

export function useIotaLedgerClient() {
    const iotaLedgerClientContext = useContext(IotaLedgerClientContext);
    if (!iotaLedgerClientContext) {
        throw new Error('useIotaLedgerClient must be used within IotaLedgerClientContext');
    }
    return iotaLedgerClientContext;
}

async function requestLedgerConnection() {
    const ledgerTransportClass = await getLedgerTransportClass();
    try {
        return await ledgerTransportClass.request();
    } catch (error) {
        throw convertErrorToLedgerConnectionFailedError(error);
    }
}

async function openLedgerConnection() {
    const ledgerTransportClass = await getLedgerTransportClass();
    let ledgerTransport: TransportWebHID | TransportWebUSB | null | undefined;

    try {
        ledgerTransport = await ledgerTransportClass.openConnected();
    } catch (error) {
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
    if (await TransportWebHID.isSupported()) {
        return TransportWebHID;
    } else if (await TransportWebUSB.isSupported()) {
        return TransportWebUSB;
    }
    throw new LedgerNoTransportMechanismError(
        "There are no supported transport mechanisms to connect to the user's Ledger device",
    );
}
