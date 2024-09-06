// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useActiveAddress } from '_src/ui/app/hooks';
import { useFormatCoin, type GasSummaryType } from '@iota/core';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota-sdk/utils';

import { KeyValueInfo } from '@iota/apps-ui-kit';
import { useAddressLink } from '_src/ui/app/hooks/useAddressLink';

interface GasSummaryProps {
    gasSummary?: GasSummaryType;
}

export function GasSummary({ gasSummary }: GasSummaryProps) {
    const [gas, symbol] = useFormatCoin(gasSummary?.totalGas, IOTA_TYPE_ARG);
    const address = useActiveAddress();
    const gasOwnerLink = useAddressLink(gasSummary?.owner || null);

    if (!gasSummary)
        return <KeyValueInfo keyText="Gas fee" valueText="0" supportingLabel={symbol} />;

    return (
        <>
            {address === gasSummary?.owner && (
                <KeyValueInfo
                    keyText="Gas fee"
                    valueText={gasSummary?.isSponsored ? '0' : gas}
                    supportingLabel={symbol}
                />
            )}
            {gasSummary?.isSponsored && gasSummary.owner && (
                <>
                    <KeyValueInfo
                        keyText="Sponsored fee"
                        valueText={gas}
                        supportingLabel={symbol}
                    />
                    <KeyValueInfo
                        keyText="Sponsor"
                        valueText={formatAddress(gasSummary.owner)}
                        valueLink={gasOwnerLink.explorerHref}
                    />
                </>
            )}
        </>
    );
}
