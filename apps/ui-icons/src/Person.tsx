// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgPerson(props: SVGProps<SVGSVGElement>) {
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
                d="M14.422 13.375a5 5 0 1 0-4.844 0A8 8 0 0 0 4 21h2a6 6 0 1 1 12 0h2a8 8 0 0 0-5.578-7.625ZM15 9a3 3 0 1 1-6 0 3 3 0 0 1 6 0Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
