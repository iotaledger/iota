// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import { useIotaClientQuery } from "@iota/dapp-kit";

/**
 * A re-usable hook for querying a locked object by ID
 * from the on-chain state.
 */
export function useGetLockedObject({ lockedId }: { lockedId: string }) {
  return useIotaClientQuery(
    "getObject",
    {
      id: lockedId,
      options: {
        showType: true,
        showOwner: true,
        showContent: true,
      },
    },
    {
      enabled: !!lockedId,
    },
  );
}
