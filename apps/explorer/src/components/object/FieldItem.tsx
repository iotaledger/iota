// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { type IotaMoveNormalizedType } from '@iota/iota-sdk/client';
import { SyntaxHighlighter } from '~/components';
import { AddressLink, Link, ObjectLink } from '~/components/ui';
import { getFieldTypeValue } from '~/lib/ui';

interface FieldItemProps {
    value: string | number | object | boolean;
    type: IotaMoveNormalizedType | '';
    objectType: string;
    truncate?: boolean;
}

const TYPE_ADDRESS = 'Address';
const TYPE_URL = '0x2::url::Url';
const TYPE_OBJECT_ID = ['0x2::object::UID', '0x2::object::ID'];

export function FieldItem({
    value,
    type,
    truncate = false,
    objectType,
}: FieldItemProps): JSX.Element {
    // for object types, use SyntaxHighlighter
    if (typeof value === 'object') {
        return <SyntaxHighlighter code={JSON.stringify(value, null, 2)} language="json" />;
    }

    const { normalizedType } = getFieldTypeValue(type, objectType);

    if (normalizedType === TYPE_ADDRESS) {
        return (
            <div className="break-all">
                <AddressLink address={value.toString()} noTruncate={!truncate} />
            </div>
        );
    }

    if (normalizedType === 'string' && TYPE_OBJECT_ID.includes(normalizedType)) {
        return (
            <div className="break-all">
                <ObjectLink objectId={value.toString()} noTruncate={!truncate} />
            </div>
        );
    }

    if (normalizedType === TYPE_URL) {
        return (
            <div className="break-all">
                <Link href={value.toString()} variant="textHeroDark">
                    {value}
                </Link>
            </div>
        );
    }

    return (
        <div className="break-all text-body-md text-neutral-40">
            {value === null || value === undefined ? null : String(value)}
        </div>
    );
}
