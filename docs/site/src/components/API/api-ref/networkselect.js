// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

import React, { useState, useEffect } from "react";
import BrowserOnly from "@docusaurus/BrowserOnly";
import { Select, MenuItem, FormControl, InputLabel } from "@mui/material";
import { StyledEngineProvider } from "@mui/material/styles";

const NETWORKS = ["Devnet", "Testnet", "Mainnet"];

const NetworkSelect = () => {
  return (
    <BrowserOnly fallback={<div>Loading...</div>}>
      {() => {
        const [selection, setSelection] = useState(() => {
          const network = localStorage.getItem("RPC");
          return network !== null ? network : "mainnet";
        });

        useEffect(() => {
          localStorage.setItem("RPC", selection);
          window.dispatchEvent(new Event("storage"));
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
      }}
    </BrowserOnly>
  );
};

export default NetworkSelect;
