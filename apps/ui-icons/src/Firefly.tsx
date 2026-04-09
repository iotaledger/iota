// Copyright (c) 2026 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import type { SVGProps } from 'react';
export default function SvgFirefly(props: SVGProps<SVGSVGElement>) {
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
                d="M6.015 3.765a4.66 4.66 0 0 1 4.407 1.206l6.225 6.164c.199.196.28.45.266.694a.92.92 0 0 1-.245.68l-6.016 6.427c-1.117 1.194-2.76 1.697-4.33 1.315-3.472-.845-4.693-5.269-2.247-7.898l.297-.319-.508-.506C1.31 8.985 2.518 4.641 6.015 3.765m.745 14.681c-2.021-.492-2.855-3.183-1.326-4.827l.282-.304a5.74 5.74 0 0 0 5.234.972l2.12-.653-3.774 4.033c-.669.715-1.631 1-2.536.78ZM18.75 3.625a3.125 3.125 0 1 1 0 6.25 3.125 3.125 0 0 1 0-6.25"
                clipRule="evenodd"
            />
        </svg>
    );
}
