// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { IotaClient } from '@iota/iota-sdk/client';
import { screen } from '@testing-library/dom';
import { render } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { useState } from 'react';

import { IotaClientProvider } from '../../src/components/IotaClientProvider.js';
import { useIotaClient, useIotaClientContext } from '../../src/index.js';

describe('IotaClientProvider', () => {
    it('renders without crashing', () => {
        render(
            <IotaClientProvider>
                <div>Test</div>
            </IotaClientProvider>,
        );
        expect(screen.getByText('Test')).toBeInTheDocument();
    });

    it('provides a IotaClient instance to its children', () => {
        const ChildComponent = () => {
            const client = useIotaClient();
            expect(client).toBeInstanceOf(IotaClient);
            return <div>Test</div>;
        };

        render(
            <IotaClientProvider>
                <ChildComponent />
            </IotaClientProvider>,
        );
    });

    it('can accept pre-configured IotaClients', () => {
        const iotaClient = new IotaClient({ url: 'http://localhost:8080' });
        const ChildComponent = () => {
            const client = useIotaClient();
            expect(client).toBeInstanceOf(IotaClient);
            expect(client).toBe(iotaClient);
            return <div>Test</div>;
        };

        render(
            <IotaClientProvider networks={{ localnet: iotaClient }}>
                <ChildComponent />
            </IotaClientProvider>,
        );

        expect(screen.getByText('Test')).toBeInTheDocument();
    });

    test('can create iota clients with custom options', async () => {
        function NetworkSelector() {
            const ctx = useIotaClientContext();

            return (
                <div>
                    {Object.keys(ctx.networks).map((network) => (
                        <button key={network} onClick={() => ctx.selectNetwork(network)}>
                            {`select ${network}`}
                        </button>
                    ))}
                </div>
            );
        }
        function CustomConfigProvider() {
            const [selectedNetwork, setSelectedNetwork] = useState<string>();

            return (
                <IotaClientProvider
                    networks={{
                        a: {
                            url: 'http://localhost:8080',
                            custom: setSelectedNetwork,
                        },
                        b: {
                            url: 'http://localhost:8080',
                            custom: setSelectedNetwork,
                        },
                    }}
                    createClient={(name, { custom, ...config }) => {
                        custom(name);
                        return new IotaClient(config);
                    }}
                >
                    <div>{`selected network: ${selectedNetwork}`}</div>
                    <NetworkSelector />
                </IotaClientProvider>
            );
        }

        const user = userEvent.setup();

        render(<CustomConfigProvider />);

        expect(screen.getByText('selected network: a')).toBeInTheDocument();

        await user.click(screen.getByText('select b'));

        expect(screen.getByText('selected network: b')).toBeInTheDocument();
    });

    test('controlled mode', async () => {
        function NetworkSelector(props: { selectNetwork: (network: string) => void }) {
            const ctx = useIotaClientContext();

            return (
                <div>
                    <div>{`selected network: ${ctx.network}`}</div>
                    {Object.keys(ctx.networks).map((network) => (
                        <button key={network} onClick={() => props.selectNetwork(network)}>
                            {`select ${network}`}
                        </button>
                    ))}
                </div>
            );
        }

        function ControlledProvider() {
            const [selectedNetwork, setSelectedNetwork] = useState<'a' | 'b'>('a');

            return (
                <IotaClientProvider
                    networks={{
                        a: {
                            url: 'http://localhost:8080',
                            custom: setSelectedNetwork,
                        },
                        b: {
                            url: 'http://localhost:8080',
                            custom: setSelectedNetwork,
                        },
                    }}
                    network={selectedNetwork}
                >
                    <NetworkSelector
                        selectNetwork={(network) => {
                            setSelectedNetwork(network as 'a' | 'b');
                        }}
                    />
                </IotaClientProvider>
            );
        }

        const user = userEvent.setup();

        render(<ControlledProvider />);

        expect(screen.getByText('selected network: a')).toBeInTheDocument();

        await user.click(screen.getByText('select b'));

        expect(screen.getByText('selected network: b')).toBeInTheDocument();
    });

    test('onNetworkChange', async () => {
        function NetworkSelector() {
            const ctx = useIotaClientContext();

            return (
                <div>
                    <div>{`selected network: ${ctx.network}`}</div>
                    {Object.keys(ctx.networks).map((network) => (
                        <button key={network} onClick={() => ctx.selectNetwork(network)}>
                            {`select ${network}`}
                        </button>
                    ))}
                </div>
            );
        }

        function ControlledProvider() {
            const [selectedNetwork, setSelectedNetwork] = useState<string>('a');

            return (
                <IotaClientProvider
                    networks={{
                        a: {
                            url: 'http://localhost:8080',
                            custom: setSelectedNetwork,
                        },
                        b: {
                            url: 'http://localhost:8080',
                            custom: setSelectedNetwork,
                        },
                    }}
                    network={selectedNetwork as 'a' | 'b'}
                    onNetworkChange={(network) => {
                        setSelectedNetwork(network);
                    }}
                >
                    <NetworkSelector />
                </IotaClientProvider>
            );
        }

        const user = userEvent.setup();

        render(<ControlledProvider />);

        expect(screen.getByText('selected network: a')).toBeInTheDocument();

        await user.click(screen.getByText('select b'));

        expect(screen.getByText('selected network: b')).toBeInTheDocument();
    });
});
