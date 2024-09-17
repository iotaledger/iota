// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Address, Badge, BadgeType, InfoBox, InfoBoxType, Panel } from '@iota/apps-ui-kit';
import { Globe, Info } from '@iota/ui-icons';
import { formatAddress } from '@iota/iota-sdk/utils';
import { Placeholder } from '@iota/ui';

type PageHeaderType = 'Transaction' | 'Checkpoint' | 'Address' | 'Object' | 'Package';

export interface PageHeaderProps {
    title: string;
    subtitle?: string | null;
    type: PageHeaderType;
    status?: 'success' | 'failure';
    after?: React.ReactNode;
    error?: string;
    loading?: boolean;
}

export function PageHeader({
    title,
    subtitle,
    type,
    error,
    loading,
    after,
    status,
}: PageHeaderProps): JSX.Element {
    return (
        <Panel>
            <div className="flex w-full items-center p-md--rs">
                <div className="flex w-full flex-row items-center justify-between gap-xs">
                    <div className="flex w-1/2 flex-col gap-xxs sm:w-3/4">
                        {loading ? (
                            <Placeholder rounded="xl" width="50%" height="10px" />
                        ) : (
                            <>
                                {type && (
                                    <div className="flex flex-row items-center gap-xxs">
                                        <span className="text-headline-sm text-neutral-10 dark:text-neutral-92">
                                            {type}
                                        </span>
                                        {status && (
                                            <Badge
                                                label={status}
                                                type={
                                                    status === 'success'
                                                        ? BadgeType.PrimarySoft
                                                        : BadgeType.Neutral
                                                }
                                            />
                                        )}
                                    </div>
                                )}
                                {title && (
                                    <div className="flex items-center gap-xxs text-neutral-40 dark:text-neutral-60">
                                        <Globe />
                                        <Address
                                            text={formatAddress(title)}
                                            isCopyable
                                            copyText={title}
                                        />
                                    </div>
                                )}
                                {subtitle && (
                                    <span className="pt-sm text-body-md text-neutral-40 dark:text-neutral-60">
                                        {subtitle}
                                    </span>
                                )}
                                {error && (
                                    <InfoBox
                                        title={error}
                                        icon={<Info />}
                                        type={InfoBoxType.Default}
                                    />
                                )}
                            </>
                        )}
                    </div>
                    {after && <div className="w-1/2 sm:w-1/4">{after}</div>}
                </div>
            </div>
        </Panel>
    );
}
