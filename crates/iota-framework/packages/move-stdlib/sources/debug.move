// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

/// Module providing debug functionality.
module std::debug;

public native fun print<T>(x: &T);

public native fun print_stack_trace();
