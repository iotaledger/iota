// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, useEffect } from "react";
import { Select, MenuItem, FormControl, InputLabel } from "@mui/material";
import { StyledEngineProvider } from "@mui/material/styles";
import ExecutionEnvironment from "@docusaurus/ExecutionEnvironment";

const NETWORKS = ["Mainnet", "Testnet", "Devnet"];

const NetworkSelect = () => {
  const [selection, setSelection] = useState(() => {

    if (ExecutionEnvironment.canUseDOM) {
      const network = localStorage.getItem("RPC");
      if (network === null) {
        return "mainnet";
      }
      return localStorage.getItem("RPC");
    } else {
      return "mainnet";
    }
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
          >{`RPC: https://api.${selection.toLowerCase()}.iota.cafe:443`}</InputLabel>
          <Select
            label-id="network"
            id="network-select"
            value={selection}
            label={`RPC: https://api.${selection.toLowerCase()}.iota.cafe:443`}
            onChange={handleChange}
            className="dark:text-white dark:bg-iota-ghost-dark"
          >
            <MenuItem value="mainnet">{NETWORKS[0]}</MenuItem>
            <MenuItem value="testnet">{NETWORKS[1]}</MenuItem>
            <MenuItem value="devnet">{NETWORKS[2]}</MenuItem>
          </Select>
        </FormControl>
      </div>
    </StyledEngineProvider>
  );
};

export default NetworkSelect;
