// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Test that `ledgerView` reports a consistent view over the backward-diff
// retention window: `min = max(backward-history watermark, latest - lookback)`,
// `max = latest`. In this test the watermark sits at 0 (migration ran before
// any checkpoint committed; no pruner advances it) and `latest` never reaches
// the lookback threshold, so the view is always `CONSISTENT` over `(0, latest)`.

//# init --protocol-version 4 --simulator

//# run-graphql
{
  ledgerView {
    kind
    min {
      digest
      sequenceNumber
    }
    max {
      digest
      sequenceNumber
    }
  }

  min: checkpoint(id: { sequenceNumber: 0 } ) {
    digest
    sequenceNumber
  }

  max: checkpoint {
    digest
    sequenceNumber
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  ledgerView {
    kind
    min {
      sequenceNumber
    }
    max {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  ledgerView {
    kind
    min {
      sequenceNumber
    }
    max {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  ledgerView {
    kind
    min {
      sequenceNumber
    }
    max {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  ledgerView {
    kind
    min {
      sequenceNumber
    }
    max {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  ledgerView {
    kind
    min {
      sequenceNumber
    }
    max {
      sequenceNumber
    }
  }

  min: checkpoint(id: { sequenceNumber: 0 } ) {
    digest
    sequenceNumber
  }

  max: checkpoint {
    digest
    sequenceNumber
  }
}
