// Copyright (c) 2025 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Query keys mostly to identify queries
// Non-identifying variables should not be included here (e.g, price of a name)
export const queryKey = {
    all: ['iota-name'],

    // Names
    nameRecord: (name: string) => [...queryKey.all, 'name-record', name],
    registerName: (name: string, address?: string) => [
        ...queryKey.all,
        'register-name',
        name,
        address,
    ],
    updateName: (address?: string) => [...queryKey.all, 'update-name', address],

    // Address
    publicName: (address: string) => [...queryKey.all, 'public-name', address],

    // Generic
    ownedObjects: (address: string) => [...queryKey.all, 'owned-objects', address],
    getObject: (name: string) => [...queryKey.all, 'get-object', name],

    // Price List
    priceList: () => [...queryKey.all, 'price-list'],
    renewalPrice: (name: string, years: number) => [...queryKey.all, 'renewal-price', name, years],

    // Auctions
    auctionList: () => [queryKey.all, 'auction-list'],
    userAuctionHistory: (address?: string) => [...queryKey.all, 'user-auction-hist', address],
    placeBid: (address: string) => [...queryKey.all, 'place-bid', address],
    auctionMetadata: (name: string) => [...queryKey.all, 'auction-metadata', name],
    claimAuction: (name: string, address?: string) => [
        ...queryKey.all,
        'claim-auction',
        name,
        address,
    ],

    // Names Config
    namesConfig: () => [...queryKey.all, 'names-config'],
    purchaseConfig: (paymentType: string, auctionType: string) => [
        ...queryKey.all,
        'purchase-config',
        paymentType,
        auctionType,
    ],

    // Deny List
    reservedList: () => [...queryKey.all, 'reserved-list'],
    blockedList: () => [...queryKey.all, 'blocked-list'],

    // Features
    methodSupported: (packageId: string, module: string, method: string) => [
        ...queryKey.all,
        'method-supported',
        packageId,
        module,
        method,
    ],
};
