// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgDarkMode(props: SVGProps<SVGSVGElement>) {
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
                fillRule="evenodd"
                clipRule="evenodd"
                d="M11.2201 6.05022C8.27486 6.43251 6 8.95059 6 12C6 15.3137 8.68629 18 12 18C13.3228 18 14.5452 17.5726 15.5374 16.847C12.3735 16.1744 10 13.3644 10 10C10 8.53557 10.4506 7.17445 11.2201 6.05022ZM4 12C4 7.58172 7.58172 4 12 4C12.7961 4 13.3232 4.54031 13.5017 5.12203C13.6694 5.66858 13.5651 6.31196 13.1586 6.79923C12.4347 7.66686 12 8.78135 12 10C12 12.7614 14.2386 15 17 15C17.5389 15 17.9878 15.3043 18.2148 15.716C18.4516 16.1457 18.4678 16.75 18.0604 17.2224C16.5949 18.9217 14.4228 20 12 20C7.58172 20 4 16.4183 4 12Z"
                fill="currentColor"
            />
        </svg>
    );
}
