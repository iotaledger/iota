// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { DEFAULT_IMAGE } from '../utils/constants';
import { OwnedObjectType } from './Inventory/OwnedObjects';

export function DisplayObjectThumbnail({ item }: { item: OwnedObjectType }) {
    return (
        <div className="flex gap-5 items-center ">
            <div className="bg-gray-100 w-[100px] h-[50px] overflow-hidden rounded my-6">
                <img
                    src={item.display.image_url}
                    className="object-contain object-center w-full h-full"
                    alt="Thumbnail of the object"
                    onError={(e) => {
                        const image = e.target as HTMLImageElement;
                        image.src = DEFAULT_IMAGE;
                    }}
                ></img>
            </div>
            <div>
                <label className="font-medium mb-1 block text-sm">Selected Item</label>
                <p>Name: {item.display.name}</p>
                <p>Description: {item.display.description}</p>
            </div>
        </div>
    );
}
