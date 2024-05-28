// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface ListProps<T extends Record<string, unknown>> {
    data: T[];
    title?: string;
    keysToShow: string[];
}

function List<T extends Record<string, unknown>>({
    data,
    title,
    keysToShow,
}: ListProps<T>): JSX.Element {
    return (
        <div className="flex flex-col gap-2">
            {title && <h2>{title}</h2>}
            <ul>
                {data.map((item) => (
                    <li key={item.id as string} className="flex gap-2">
                        {keysToShow.map((key) => (
                            <div key={key} className="flex">
                                <div>{key}:</div>
                                <div>{item[key] as React.ReactNode}</div>
                            </div>
                        ))}
                    </li>
                ))}
            </ul>
        </div>
    );
}

export default List;
