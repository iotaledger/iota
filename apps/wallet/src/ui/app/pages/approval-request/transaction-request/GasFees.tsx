// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { TitleSize, Badge, BadgeType, Title, Panel } from '@iota/apps-ui-kit';
import { Collapsible } from '_src/ui/app/shared/collapse';
import { GasSummary } from '_src/ui/app/shared/transaction-summary/cards/GasSummary';
import { type GasSummaryType } from '@iota/core';

interface GasFeesProps {
    sender?: string;
    gasSummary?: GasSummaryType;
    isEstimate?: boolean;
    isPending?: boolean;
    isError?: boolean;
}
const DEFAULT_TITLE = 'Gas Fees';

export function GasFees({ sender, gasSummary, isEstimate, isPending, isError }: GasFeesProps) {
    const title = isEstimate ? `Est. ${DEFAULT_TITLE}` : DEFAULT_TITLE;
    const trailingElement =
        gasSummary?.isSponsored && gasSummary.owner ? (
            <div className="ml-1 flex">
                <Badge type={BadgeType.PrimarySoft} label="Sponsored" />
            </div>
        ) : null;
    return (
        <Panel hasBorder>
            <div className="flex flex-col gap-y-sm overflow-hidden rounded-xl">
                <Collapsible
                    hideBorder
                    defaultOpen
                    render={() => (
                        <Title
                            size={TitleSize.Small}
                            title={title}
                            trailingElement={trailingElement}
                        />
                    )}
                >
                    <div className="flex flex-col gap-y-sm p-md">
                        <GasSummary
                            sender={sender}
                            gasSummary={gasSummary}
                            isPending={isPending}
                            isError={isError}
                        />
                    </div>
                </Collapsible>
            </div>
        </Panel>
    );
}
