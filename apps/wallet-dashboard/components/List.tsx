// Copyright (c) 2024 IOTA Stiftung
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';
import { Button } from '.';

interface ListProps {
    data: { id: string; [key: string]: React.ReactNode }[];
    title?: string;
    onItemClick?: (item: { id: string; [key: string]: React.ReactNode }) => void;
    actionText?: string;
}

function List({ data, title, onItemClick, actionText }: ListProps): JSX.Element {
    return (
        <div className="flex flex-col gap-2">
            {title && <h2>{title}</h2>}
            <ul className="flex flex-col gap-2">
                {data.map((item) => (
                    <li key={item.id} className="flex items-center gap-2">
                        {Object.entries(item).map(([key, value]) => (
                            <div key={key} className="flex">
                                <div>{key}:</div>
                                <div>{value}</div>
                            </div>
                        ))}
                        {onItemClick && (
                            <Button onClick={() => onItemClick(item)}>{actionText}</Button>
                        )}
                    </li>
                ))}
            </ul>
        </div>
    );
}

export default List;
