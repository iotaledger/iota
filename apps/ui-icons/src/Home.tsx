// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgHome(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path fill="currentColor" d="M12 14a2 2 0 1 0 0-4 2 2 0 0 0 0 4Z" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M4 10a2 2 0 0 1 .8-1.6l6-4.5a2 2 0 0 1 2.4 0l6 4.5A2 2 0 0 1 20 10v9a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2v-9Zm14 0v9H6v-9l6-4.5 6 4.5Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
