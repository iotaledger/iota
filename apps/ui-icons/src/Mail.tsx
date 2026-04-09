// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgMail(props: SVGProps<SVGSVGElement>) {
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
                d="M18.206 4.005A4 4 0 0 1 22 8v8l-.005.206a4 4 0 0 1-3.789 3.79L18 20H6l-.206-.005A4 4 0 0 1 2 16V8a4 4 0 0 1 3.794-3.995L6 4h12zm-4.23 8.596a3 3 0 0 1-3.733.173l-.219-.173L4.09 7.406A2 2 0 0 0 4 8v8a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8a2 2 0 0 0-.09-.594zM6 6q-.22.001-.428.047l5.77 5.048.073.058a1 1 0 0 0 1.244-.058l5.768-5.048A2 2 0 0 0 18 6z"
                clipRule="evenodd"
            />
        </svg>
    );
}
