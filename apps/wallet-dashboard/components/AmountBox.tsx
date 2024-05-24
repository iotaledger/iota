// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { Box } from "@/components/index"

interface AmountBoxProps {
  title: string
  amount: string
}

function AmountBox({ title, amount }: AmountBoxProps): JSX.Element {

    return (
        <div className="flex gap-4 items-center justify-center pt-12">
            <Box title={title}>
                <p>{amount}</p>
            </Box>
           
        </div>
    )
}

export default AmountBox