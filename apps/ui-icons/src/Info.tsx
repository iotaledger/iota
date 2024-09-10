// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgInfo(props: SVGProps<SVGSVGElement>) {
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
                d="M12 21a9 9 0 1 0 0-18 9 9 0 0 0 0 18Zm0-14.4A1.2 1.2 0 1 0 12 9a1.2 1.2 0 0 0 0-2.4Zm1.2 9a1.2 1.2 0 0 1-2.4 0V12a1.2 1.2 0 0 1 2.4 0v3.6Z"
                clipRule="evenodd"
            />
        </svg>
    );
}
