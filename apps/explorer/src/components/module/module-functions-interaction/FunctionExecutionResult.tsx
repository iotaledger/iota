// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { LinkGroup } from './LinkGroup';
import type { IotaTransactionBlockResponse, OwnedObjectRef } from '@iota/iota-sdk/client';
import { Close } from '@iota/apps-ui-icons';

interface ToObjectLink {
    text: string;
    to: string;
}

function toObjectLink(object: OwnedObjectRef): ToObjectLink {
    return {
        text: object.reference.objectId,
        to: `/object/${encodeURIComponent(object.reference.objectId)}`,
    };
}

type FunctionExecutionResultProps = {
    result: IotaTransactionBlockResponse | null;
    error: string | false;
    onClear: () => void;
};

export function FunctionExecutionResult({ error, result, onClear }: FunctionExecutionResultProps) {
    const adjError = error || (result && result.effects?.status.error) || null;
    return (
        <div className="relative inline-flex flex-nowrap items-center gap-2 overflow-hidden rounded-lg bg-neutral-96 p-xs dark:bg-neutral-12">
            <div className="space-y-4 text-body-sm">
                <LinkGroup
                    title="Digest"
                    links={
                        result
                            ? [
                                  {
                                      text: result.digest,
                                      to: `/txblock/${encodeURIComponent(result.digest)}`,
                                  },
                              ]
                            : []
                    }
                />
                <LinkGroup
                    title="Created"
                    links={(result && result.effects?.created?.map(toObjectLink)) || []}
                />
                <LinkGroup
                    title="Updated"
                    links={(result && result.effects?.mutated?.map(toObjectLink)) || []}
                />
                <LinkGroup title="Transaction failed" text={adjError} />
                <div className="absolute right-2 top-2">
                    <Close
                        className="h-3 w-3 text-neutral-10 dark:text-neutral-92"
                        onClick={onClear}
                        aria-label="Close"
                    />
                </div>
            </div>
        </div>
    );
}
