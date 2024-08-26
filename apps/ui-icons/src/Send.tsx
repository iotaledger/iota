// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgSend(props: SVGProps<SVGSVGElement>) {
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
                d="m12.013 21.389-2.664-6.717-6.718-2.664c-.848-.337-.84-1.541.014-1.865l16.018-6.076c.806-.305 1.596.484 1.29 1.29l-6.076 16.018c-.324.854-1.528.863-1.864.014Zm-3.069-9.03 4.091-2.045a.5.5 0 0 1 .671.67l-2.045 4.092 1.26 3.18 4.374-11.53-11.53 4.373 3.18 1.26Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
