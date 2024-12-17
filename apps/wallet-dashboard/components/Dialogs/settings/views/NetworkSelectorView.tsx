// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Header, RadioButton } from '@iota/apps-ui-kit';
import { DialogLayout, DialogLayoutBody } from '../../layout';
import { NetworkConfiguration } from '@iota/iota-sdk/client';
import { useIotaClientContext } from '@iota/dapp-kit';
import toast from 'react-hot-toast';

interface NetworkSelectorViewProps {
    handleClose: () => void;
    onBack: () => void;
}

export function NetworkSelectorView({
    handleClose,
    onBack,
}: NetworkSelectorViewProps): JSX.Element {
    const clientContext = useIotaClientContext();
    const activeNetwork = clientContext.network;

    async function handleNetworkChange(network: NetworkConfiguration) {
        if (activeNetwork === network.id) {
            return;
        }
        clientContext.selectNetwork(network.id);
        toast.success(`Switched to ${network.name}`);
    }
    return (
        <DialogLayout>
            <Header title="Network" onClose={handleClose} onBack={onBack} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    {Object.keys(clientContext.networks).map((network) => {
                        const networkConfig = clientContext.networks[
                            network
                        ] as NetworkConfiguration;
                        return (
                            <div className="px-md" key={networkConfig.id}>
                                <RadioButton
                                    label={networkConfig.name}
                                    isChecked={activeNetwork === networkConfig.id}
                                    onChange={() => handleNetworkChange(networkConfig)}
                                />
                            </div>
                        );
                    })}
                </div>
            </DialogLayoutBody>
        </DialogLayout>
    );
}
