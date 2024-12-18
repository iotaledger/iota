// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useResolveIotaNSName } from '_app/hooks/useAppResolveIotaNSName';
import { Text } from '_src/ui/app/shared/text';

import { TxnAddressLink } from './TxnAddressLink';

type TxnAddressProps = {
    address: string;
    label: string;
};

export function TxnAddress({ address, label }: TxnAddressProps) {
    const domainName = useResolveIotaNSName(address);

    return (
        <div className="flex w-full items-center justify-between py-3.5 first:pt-0">
            <Text variant="body" weight="medium" color="steel-darker">
                {label}
            </Text>
            <div className="flex items-center gap-1">
                <TxnAddressLink address={domainName ?? address} />
            </div>
        </div>
    );
}
