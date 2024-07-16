// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgEdit(props: SVGProps<SVGSVGElement>) {
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
                d="M19.561 4.439a1.503 1.503 0 0 0-2.122 0l-.791.789 2.12 2.116.793-.792a1.492 1.492 0 0 0 0-2.113Zm1.413 3.528a3.492 3.492 0 0 0 0-4.944 3.503 3.503 0 0 0-4.948 0l-1.5 1.497-.002.001L2 17.021V22h4.914l14.06-14.033Zm-3.622.79-2.12-2.116L4 17.85V20h2.086L17.352 8.756Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
