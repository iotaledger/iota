// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { ExplorerLink, ExplorerLinkType } from '_components';
import { useActiveAddress } from '_src/ui/app/hooks';
import { useFormatCoin, type GasSummaryType } from '@iota/core';
import { formatAddress, IOTA_TYPE_ARG } from '@iota/iota.js/utils';

import { Text } from '../../text';

interface GasSummaryProps {
    gasSummary?: GasSummaryType;
}

export function GasSummary({ gasSummary }: GasSummaryProps) {
    const [gas, symbol] = useFormatCoin(gasSummary?.totalGas, IOTA_TYPE_ARG);
    const address = useActiveAddress();

    if (!gasSummary) return null;

    return (
        <div className="relative flex flex-col rounded-2xl bg-white shadow-card-soft">
            <div className="bg-gray-40 rounded-t-2xl px-4 py-2.5">
                <Text color="steel-darker" variant="captionSmall" weight="semibold">
                    Gas Fees
                </Text>
            </div>
            <div className="flex w-full flex-col items-center gap-2 px-4 py-3">
                <div className="flex w-full items-center justify-start">
                    {address === gasSummary?.owner && (
                        <div className="mr-auto">
                            <Text color="steel-dark" variant="pBody" weight="medium">
                                You Paid
                            </Text>
                        </div>
                    )}
                    <Text color="steel-darker" variant="pBody" weight="medium">
                        {gasSummary?.isSponsored ? '0' : gas} {symbol}
                    </Text>
                </div>
                {gasSummary?.isSponsored && gasSummary.owner && (
                    <>
                        <div className="flex w-full justify-between">
                            <Text color="steel-dark" variant="pBody" weight="medium">
                                Paid by Sponsor
                            </Text>
                            <Text color="steel-darker" variant="pBody" weight="medium">
                                {gas} {symbol}
                            </Text>
                        </div>
                        <div className="flex w-full justify-between">
                            <Text color="steel-dark" variant="pBody" weight="medium">
                                Sponsor
                            </Text>
                            <ExplorerLink
                                type={ExplorerLinkType.Address}
                                address={gasSummary.owner}
                                className="text-hero-dark no-underline"
                            >
                                <Text variant="pBodySmall" truncate mono>
                                    {formatAddress(gasSummary.owner)}
                                </Text>
                            </ExplorerLink>
                        </div>
                    </>
                )}
            </div>
        </div>
    );
}
