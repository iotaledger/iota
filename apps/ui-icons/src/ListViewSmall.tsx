// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgListViewSmall(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path
                fill="currentColor"
                d="M9 5a1 1 0 0 0 0 2h10a1 1 0 1 0 0-2H9ZM9 11a1 1 0 1 0 0 2h10a1 1 0 1 0 0-2H9ZM8 18a1 1 0 0 1 1-1h10a1 1 0 1 1 0 2H9a1 1 0 0 1-1-1ZM5 11a1 1 0 1 0 0 2 1 1 0 0 0 0-2ZM4 6a1 1 0 1 1 2 0 1 1 0 0 1-2 0ZM5 17a1 1 0 1 0 0 2 1 1 0 0 0 0-2Z"
            />
        </svg>
    );
}
