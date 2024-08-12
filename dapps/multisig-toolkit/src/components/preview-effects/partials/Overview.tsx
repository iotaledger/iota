// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientContext } from '@iota/dapp-kit';
import { DryRunTransactionBlockResponse, GasCostSummary } from '@iota/iota-sdk/src/client';
import { ReactNode } from 'react';

import { ObjectLink } from '../ObjectLink';
import { onChainAmountToFloat } from '../utils';

const calculateGas = (gas: GasCostSummary): string => {
    return (
        onChainAmountToFloat(
            (
                BigInt(gas.computationCost) +
                BigInt(gas.storageCost) -
                BigInt(gas.storageRebate)
            ).toString(),
            9,
        )?.toString() || '-'
    );
};

export function Overview({ output }: { output: DryRunTransactionBlockResponse }) {
    const { network } = useIotaClientContext();
    const metadata: Record<string, ReactNode> = {
        network,
        status:
            output.effects.status?.status === 'success'
                ? '✅ Transaction dry run executed successfully!'
                : output.effects.status?.status === 'failure'
                  ? '❌ Transaction failed to execute!'
                  : null,

        sender: (
            <span className="flex gap-2 items-center">
                <ObjectLink
                    owner={{
                        AddressOwner: output.input.sender,
                    }}
                />
            </span>
        ),
        epoch: output.effects.executedEpoch,
        gas: calculateGas(output.effects.gasUsed) + ' IOTA',
    };

    return (
        <div className="border p-3 w-full rounded">
            {Object.entries(metadata).map(([key, value]) => (
                <div key={key} className="flex items-center gap-3 ">
                    <span className="capitalize">{key}: </span>
                    {value}
                </div>
            ))}
        </div>
    );
}
