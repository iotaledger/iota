// Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export function* indexGenerator(cap: number): Generator<number, number, number> {
    let index = 0;
    while (true) {
        if (index >= cap) {
            index = 0;
            yield index++;
        } else {
            yield index++;
        }
    }
}
