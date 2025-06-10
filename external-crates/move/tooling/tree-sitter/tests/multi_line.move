// Copyright (c) The Move Contributors
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

module a::m {
    friend // why
    a::n;
    public( // why folks, why
        friend
    ) fun t() {}

    public( // why folks, why
        package
    ) entry fun t() {}
}
