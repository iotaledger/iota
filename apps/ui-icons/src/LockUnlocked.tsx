// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgLockUnlocked(props: SVGProps<SVGSVGElement>) {
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
                d="M8 11V8.104C8 6.5 9.577 5 11.5 5S15 6.5 15 8h2c0-2.82-2.69-5-5.5-5S6 5.285 6 8.104V11c-1.183 0-2 .962-2 2.15v5.7C4 20.039 4.96 21 6.143 21h10.714A2.146 2.146 0 0 0 19 18.85v-5.7c0-1.188-.96-2.15-2.143-2.15H8Zm5.321 5.272c0 .95-.767 1.72-1.714 1.72s-1.714-.77-1.714-1.72.767-1.72 1.714-1.72 1.714.77 1.714 1.72Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
