// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgRecognizedBadge(props: SVGProps<SVGSVGElement>) {
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
                d="M14.882 5.043 12 2 9.118 5.043l-4.19-.114.115 4.19L2 12l3.043 2.882-.114 4.19 4.19-.115L12 22l2.882-3.043 4.19.114-.115-4.19L22 12l-3.043-2.882.114-4.19zm1.865 4.621a1 1 0 0 0-1.494-1.328l-4.63 5.207-1.916-1.917a1 1 0 0 0-1.414 1.414l2.667 2.667a1 1 0 0 0 1.454-.043z"
                clipRule="evenodd"
            />
        </svg>
    );
}
