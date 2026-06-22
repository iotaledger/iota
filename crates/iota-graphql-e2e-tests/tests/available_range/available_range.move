// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

// Test that `availableRange` reports the backward-diff retention window:
// `first = max(backward-history watermark, latest - lookback)`, `last = latest`.
// In this test the watermark sits at 0 (migration ran before any checkpoint
// committed; no pruner advances it) and `latest` never reaches the lookback
// threshold, so the range is always `(0, latest)`.

//# init --protocol-version 4 --simulator

//# run-graphql
{
  availableRange {
    first {
      digest
      sequenceNumber
    }
    last {
      digest
      sequenceNumber
    }
  }

  first: checkpoint(id: { sequenceNumber: 0 } ) {
    digest
    sequenceNumber
  }

  last: checkpoint {
    digest
    sequenceNumber
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  availableRange {
    first {
      sequenceNumber
    }
    last {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  availableRange {
    first {
      sequenceNumber
    }
    last {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  availableRange {
    first {
      sequenceNumber
    }
    last {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  availableRange {
    first {
      sequenceNumber
    }
    last {
      sequenceNumber
    }
  }
}

//# advance-clock --duration-ns 1

//# create-checkpoint

//# run-graphql
{
  availableRange {
    first {
      sequenceNumber
    }
    last {
      sequenceNumber
    }
  }

  first: checkpoint(id: { sequenceNumber: 0 } ) {
    digest
    sequenceNumber
  }

  last: checkpoint {
    digest
    sequenceNumber
  }
}
