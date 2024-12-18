// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0
import { SVGProps } from 'react';
export default function SvgDisconnect(props: SVGProps<SVGSVGElement>) {
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
                d="m12 19.071 1.828-1.828-1.414-1.414-1.828 1.828q-.885.885-2.122.884a2.9 2.9 0 0 1-2.12-.884 2.9 2.9 0 0 1-.885-2.121q0-1.237.884-2.122l1.829-1.828-1.415-1.414L4.93 12q-1.467 1.467-1.467 3.536c0 2.069.489 2.557 1.467 3.535q1.467 1.467 3.535 1.467 2.07 0 3.536-1.467Zm3.828-6.657 1.415 1.415L19.07 12q1.467-1.467 1.467-3.535 0-2.07-1.467-3.536t-3.536-1.467q-2.068 0-3.535 1.467l-1.828 1.829 1.414 1.414 1.828-1.829a2.9 2.9 0 0 1 2.122-.884 2.9 2.9 0 0 1 2.12.884q.885.885.885 2.122c0 1.237-.295 1.532-.884 2.121zM4.293 5.707l14 14 1.414-1.414-14-14z"
            />
        </svg>
    );
}
