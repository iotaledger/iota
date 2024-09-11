// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgMenuIcon(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path d="M0 0h24v24H0z" />
            <path d="M-282-881h948V200h-948z" />
            <g fill="currentColor">
                <path d="M21 9a1 1 0 0 1-1 1H4a1 1 0 1 1 0-2h16a1 1 0 0 1 1 1ZM21 15a1 1 0 0 1-1 1H10a1 1 0 1 1 0-2h10a1 1 0 0 1 1 1Z" />
            </g>
        </svg>
    );
}
