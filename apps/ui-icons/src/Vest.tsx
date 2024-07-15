// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgVest(props: SVGProps<SVGSVGElement>) {
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
                fillRule="evenodd"
                d="M7.708 6.293a3 3 0 1 0-1.414 1.414l1.534 1.536a5 5 0 0 0 6.929 6.929l1.535 1.535a3 3 0 1 0 1.414-1.414l-1.534-1.536a5 5 0 0 0-6.929-6.929L7.708 6.293ZM4 5a1 1 0 1 1 2 0 1 1 0 0 1-2 0Zm8 10a3 3 0 1 0 0-6 3 3 0 0 0 0 6Zm7 3a1 1 0 1 0 0 2 1 1 0 0 0 0-2Z"
                clipRule="evenodd"
            />
            <path
                fill="currentColor"
                d="M19 7a2 2 0 1 0 0-4 2 2 0 0 0 0 4ZM7 19a2 2 0 1 1-4 0 2 2 0 0 1 4 0Z"
            />
        </svg>
    );
}
