// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Box } from '@/components/index';

interface AmountBoxProps {
    title: string;
    amount: string;
}

export function AmountBox({ title, amount }: AmountBoxProps): JSX.Element {
    return (
        <div className="flex items-center justify-center gap-4 pt-12">
            <Box title={title}>
                <p>{amount}</p>
            </Box>
        </div>
    );
}
