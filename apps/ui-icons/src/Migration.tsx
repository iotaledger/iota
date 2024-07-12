// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { SVGProps } from 'react';
export default function SvgMigration(props: SVGProps<SVGSVGElement>) {
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
                d="M17.72 3.139c-.266-.297-.72-.087-.72.333V6h-1a6 6 0 0 0-6 6 4 4 0 0 1-4 4H3v2h3a6 6 0 0 0 6-6 4 4 0 0 1 4-4h1v2.528c0 .42.454.63.72.333l3.157-3.528a.511.511 0 0 0 0-.666L17.72 3.139Z"
            />
            <path
                fill="currentColor"
                d="M17.72 13.139c-.266-.297-.72-.087-.72.333V16h-1a3.992 3.992 0 0 1-3.139-1.52 3.504 3.504 0 0 1-1.14 1.726A5.982 5.982 0 0 0 15.999 18H17v2.528c0 .42.454.63.72.333l3.157-3.528a.511.511 0 0 0 0-.666l-3.157-3.528ZM10.28 7.795A5.981 5.981 0 0 0 6 6H3v2h3c1.273 0 2.407.594 3.139 1.52.2-.685.603-1.284 1.14-1.725Z"
            />
        </svg>
    );
}
