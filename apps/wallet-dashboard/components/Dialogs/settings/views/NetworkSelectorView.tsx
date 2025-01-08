// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Header, RadioButton } from '@iota/apps-ui-kit';
import { DialogLayout, DialogLayoutBody } from '../../layout';
import { NetworkConfiguration } from '@iota/iota-sdk/client';
import { useIotaClientContext } from '@iota/dapp-kit';
import { usePersistedNetwork } from '@/hooks';

interface NetworkSelectorViewProps {
    handleClose: () => void;
    onBack: () => void;
}

export function NetworkSelectorView({
    handleClose,
    onBack,
}: NetworkSelectorViewProps): JSX.Element {
    const clientContext = useIotaClientContext();
    // Dashboard doesn't support RPCs yet
    const networks = clientContext.networks as Record<string, NetworkConfiguration>;

    const { persistedNetwork, handleNetworkChange } = usePersistedNetwork();

    return (
        <DialogLayout>
            <Header title="Network" onClose={handleClose} onBack={onBack} titleCentered />
            <DialogLayoutBody>
                <div className="flex w-full flex-col gap-md">
                    {Object.values(networks).map((network) => (
                        <div className="px-md" key={network.id}>
                            <RadioButton
                                label={network.name}
                                isChecked={persistedNetwork === network.id}
                                onChange={() => handleNetworkChange(network)}
                            />
                        </div>
                    ))}
                </div>
            </DialogLayoutBody>
        </DialogLayout>
    );
}
