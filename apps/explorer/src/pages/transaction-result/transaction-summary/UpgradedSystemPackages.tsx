// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useState } from 'react';
import { CollapsibleCard, ObjectLink } from '~/components/ui';
import {
    Accordion,
    AccordionContent,
    AccordionHeader,
    KeyValueInfo,
    Title,
    TitleSize,
} from '@iota/apps-ui-kit';
import type { OwnedObjectRef } from '@iota/iota-sdk/client';

export function UpgradedSystemPackages({ data }: { data: OwnedObjectRef[] }): JSX.Element | null {
    const [isExpanded, setIsExpanded] = useState(true);
    if (!data?.length) return null;

    return (
        <CollapsibleCard title="Changes">
            <div className="px-md--rs pb-lg pt-xs">
                <Accordion>
                    <AccordionHeader
                        isExpanded={isExpanded}
                        onToggle={() => setIsExpanded(!isExpanded)}
                    >
                        <Title size={TitleSize.Small} title="Updated" />
                    </AccordionHeader>
                    <AccordionContent isExpanded={isExpanded}>
                        <div className="flex flex-col gap-2">
                            {data.map((object) => {
                                const { objectId } = object.reference;
                                return (
                                    <div
                                        className="flex flex-col gap-y-sm px-md--rs py-xs"
                                        key={objectId}
                                    >
                                        <KeyValueInfo
                                            keyText="Package"
                                            value={<ObjectLink objectId={objectId} />}
                                        />
                                    </div>
                                );
                            })}
                        </div>
                    </AccordionContent>
                </Accordion>
            </div>
        </CollapsibleCard>
    );
}
