// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

export interface RegistrationNft {
    name: string;
    description?: string;
    imageUrl?: string;
    link?: string;
    projectUrl?: string;
    expirationDate: Date;
    isExpired: boolean;
    isSubname: boolean;
    id: string;
}
