// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { ChevronRight12 } from '@iota/icons';
import { type IotaEvent } from '@iota/iota-sdk/client';
import { formatAddress, parseStructTag } from '@iota/iota-sdk/utils';
import { Text } from '@iota/ui';
import * as Collapsible from '@radix-ui/react-collapsible';
import clsx from 'clsx';
import { useState } from 'react';

import { SyntaxHighlighter } from '~/components';
import { CopyToClipboard, DescriptionItem, Divider, ObjectLink } from '~/components/ui';

function Event({ event, divider }: { event: IotaEvent; divider: boolean }): JSX.Element {
    const [open, setOpen] = useState(false);
    const { address, module, name } = parseStructTag(event.type);
    const objectLinkLabel = [formatAddress(address), module, name].join('::');

    return (
        <div>
            <div className="flex flex-col gap-3">
                <DescriptionItem title="Type" align="start" labelWidth="sm">
                    <Text variant="pBody/medium" color="steel-darker">
                        {objectLinkLabel}
                    </Text>
                </DescriptionItem>

                <DescriptionItem title="Event Emitter" align="start" labelWidth="sm">
                    <div className="flex items-center gap-1">
                        <ObjectLink
                            objectId={event.packageId}
                            queryStrings={{ module: event.transactionModule }}
                            label={`${formatAddress(event.packageId)}::${event.transactionModule}`}
                        />
                        <CopyToClipboard color="steel" copyText={event.packageId} />
                    </div>
                </DescriptionItem>

                <Collapsible.Root open={open} onOpenChange={setOpen} asChild>
                    <>
                        <Collapsible.Trigger className="flex cursor-pointer items-center gap-1.5">
                            <Text variant="body/semibold" color="steel-dark">
                                {open ? 'Hide' : 'View'} Event Data
                            </Text>

                            <ChevronRight12
                                className={clsx('h-3 w-3 text-steel-dark', open && 'rotate-90')}
                            />
                        </Collapsible.Trigger>

                        <Collapsible.Content className="rounded-lg border border-transparent bg-white p-5">
                            <SyntaxHighlighter
                                code={JSON.stringify(event, null, 2)}
                                language="json"
                            />
                        </Collapsible.Content>
                    </>
                </Collapsible.Root>
            </div>

            {divider && (
                <div className="my-6">
                    <Divider />
                </div>
            )}
        </div>
    );
}

interface EventsProps {
    events: IotaEvent[];
}

export function Events({ events }: EventsProps) {
    return (
        <div>
            {events.map((event, index) => (
                <Event key={event.type} event={event} divider={index !== events.length - 1} />
            ))}
        </div>
    );
}
