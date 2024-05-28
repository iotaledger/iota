// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React from 'react';

interface ListProps {
    data: Record<string, React.ReactNode>[];
    title?: string;
}

function List({ data, title }: ListProps): JSX.Element {
    return (
        <div className="flex flex-col gap-2">
            {title && <h2>{title}</h2>}
            <ul>
                {data.map((item) => (
                    <li key={item?.id as string} className="flex gap-2">
                        {Object.entries(item).map(([key, value]) => (
                            <div key={key} className="flex">
                                <div>{key}:</div>
                                <div>{value}</div>
                            </div>
                        ))}
                    </li>
                ))}
            </ul>
        </div>
    );
}

export default List;
