// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgSidePanel(props: SVGProps<SVGSVGElement>) {
    return (
        <svg
            xmlns="http://www.w3.org/2000/svg"
            width="1em"
            height="1em"
            fill="none"
            viewBox="0 0 24 24"
            {...props}
        >
            <path fill="currentColor" d="m13 12-5 4V8z" />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M19.614 3c1.324 0 2.4 1.076 2.4 2.4L22 18.652C22 19.977 20.923 21 19.6 21l-15.2-.054c-1.323 0-2.4-1.023-2.4-2.346l.015-13.253c0-1.324 1.075-2.4 2.4-2.4zM4 18.999h11V5H4zm13 0h3V5h-3z"
                clipRule="evenodd"
            />
        </svg>
    );
}
