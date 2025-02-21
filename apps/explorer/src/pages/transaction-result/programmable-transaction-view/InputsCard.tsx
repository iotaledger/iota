// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { KeyValueInfo, TitleSize } from '@iota/apps-ui-kit';
import { type IotaCallArg } from '@iota/iota-sdk/client';
import { toHEX } from '@iota/iota-sdk/utils';
import { ProgrammableTxnBlockCard, AddressLink, ObjectLink, CollapsibleCard } from '~/components';
import { useBreakpoint } from '~/hooks';

const REGEX_NUMBER = /^\d+$/;

interface InputsCardProps {
    inputs: IotaCallArg[];
}

export function InputsCard({ inputs }: InputsCardProps): JSX.Element | null {
    const isMediumOrAbove = useBreakpoint('md');
    if (!inputs?.length) {
        return null;
    }

    const expandableItems = inputs.map((input, index) => (
        <CollapsibleCard
            key={index}
            title={`Input ${index}`}
            collapsible
            titleSize={TitleSize.Small}
        >
            <div
                data-testid="inputs-card-content"
                className="flex flex-col gap-2 px-md pb-lg pt-xs"
            >
                {Object.entries(input).map(([key, value]) => {
                    let renderValue;
                    const stringValue = String(value);

                    if (key === 'mutable') {
                        renderValue = String(value);
                    } else if (key === 'objectId') {
                        renderValue = <ObjectLink objectId={stringValue} />;
                    } else if (
                        'valueType' in input &&
                        'value' in input &&
                        input.valueType === 'address' &&
                        key === 'value'
                    ) {
                        renderValue = <AddressLink address={stringValue} />;
                    } else if (REGEX_NUMBER.test(stringValue)) {
                        const bigNumber = BigInt(stringValue);
                        renderValue = bigNumber.toLocaleString();
                    } else if (
                        'valueType' in input &&
                        'value' in input &&
                        input.valueType === 'vector<u8>' &&
                        key === 'value'
                    ) {
                        let parsedVector: Array<number> | null = null;
                        try {
                            parsedVector = JSON.parse(`[${stringValue}]`);
                        } catch (_) {
                            // Silent error
                        }

                        let parsedUtf: string | null = null;
                        try {
                            parsedUtf = new TextDecoder('utf-8', {
                                fatal: true,
                            }).decode(new Uint8Array(parsedVector ?? []));
                        } catch (_) {
                            // Silent error
                        }

                        if (parsedUtf) {
                            renderValue = parsedUtf;
                        } else if (parsedVector) {
                            renderValue = toHEX(new Uint8Array(parsedVector));
                        } else {
                            renderValue = stringValue;
                        }
                    } else {
                        renderValue = stringValue;
                    }

                    return (
                        <KeyValueInfo
                            key={key}
                            keyText={key}
                            value={renderValue}
                            fullwidth={!isMediumOrAbove}
                        />
                    );
                })}
            </div>
        </CollapsibleCard>
    ));

    return (
        <ProgrammableTxnBlockCard
            initialClose
            items={expandableItems}
            itemsLabel={inputs.length > 1 ? 'Inputs' : 'Input'}
            count={inputs.length}
        />
    );
}
