// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, useEffect } from "react";
import { Select, MenuItem, FormControl, InputLabel } from "@mui/material";
import { StyledEngineProvider } from "@mui/material/styles";

const NETWORKS = ["Devnet", "Testnet", "Mainnet"];

const NetworkSelect = () => {
  const [selection, setSelection] = useState(() => {
    // Attempt to access localStorage, default to 'mainnet' if not set
    const network = typeof window !== "undefined" ? localStorage.getItem("RPC") : null;
    return network !== null ? network : "mainnet";
  });

  useEffect(() => {
    if (typeof window !== "undefined") {
      localStorage.setItem("RPC", selection);
      window.dispatchEvent(new Event("storage"));
    }
  }, [selection]);

  const handleChange = (e) => {
    setSelection(e.target.value);
  };

  return (
    <StyledEngineProvider injectFirst>
      <div className="w-11/12 pb-3">
        <FormControl fullWidth>
          <InputLabel
            id="network"
            className="dark:text-white"
          >{`RPC: https://fullnode.${selection.toLowerCase()}.iota.io:443`}</InputLabel>
          <Select
            label-id="network"
            id="network-select"
            value={selection}
            label={`RPC: https://fullnode.${selection.toLowerCase()}.iota.io:443`}
            onChange={handleChange}
            className="dark:text-white dark:bg-iota-ghost-dark"
          >
            <MenuItem value="devnet">{NETWORKS[0]}</MenuItem>
            <MenuItem value="testnet">{NETWORKS[1]}</MenuItem>
            <MenuItem value="mainnet">{NETWORKS[2]}</MenuItem>
          </Select>
        </FormControl>
      </div>
    </StyledEngineProvider>
  );
};

export default NetworkSelect;
