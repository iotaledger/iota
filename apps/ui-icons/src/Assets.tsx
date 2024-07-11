// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgAssets(props: SVGProps<SVGSVGElement>) {
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
                d="M19.5 11.959v.082a1.5 1.5 0 0 0 3 .017v-.116a1.5 1.5 0 1 0-3 .017m-2.167-5.233a1.5 1.5 0 1 0 2.133-2.11l-.041-.04-.042-.042a1.5 1.5 0 1 0-2.11 2.133l.03.03.03.03Zm2.133 12.657a1.5 1.5 0 0 0-2.133-2.11l-.03.03-.03.03a1.5 1.5 0 1 0 2.11 2.133l.042-.041.04-.042ZM12.041 4.5a1.5 1.5 0 0 0 .017-3H12a1.5 1.5 0 0 0 0 3zm.017 18a1.5 1.5 0 1 0-.017-3H12a1.5 1.5 0 0 0 0 3z"
            />
            <path
                fill="currentColor"
                fillRule="evenodd"
                d="M6 12a6 6 0 1 1 12 0 6 6 0 0 1-12 0m6 4a4 4 0 1 1 0-8 4 4 0 0 1 0 8"
                clipRule="evenodd"
            />
        </svg>
    );
}
