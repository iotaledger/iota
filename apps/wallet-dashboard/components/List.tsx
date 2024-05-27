// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface ListProps {
    data: (unknown | Record<string, unknown>)[];
    title?: string;
    keysToShow: string[];
}

function List({ data, title, keysToShow }: ListProps): JSX.Element {
    return (
        <div className="flex flex-col gap-2">
            {title && <h2>{title}</h2>}
            <ul>
                {(data as Record<string, unknown>[]).map((item, index) => (
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
