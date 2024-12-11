import React, { useState } from "react";

const NetworkDropdown = () => {
  const [selectedNetwork, setSelectedNetwork] = useState("mainnet");

  const networks = [
    { label: "Mainnet", value: "mainnet" },
    { label: "Testnet", value: "testnet" },
    { label: "Devnet", value: "devnet" },
  ];

  const handleNetworkChange = (event) => {
    const selectedValue = event.target.value;
    setSelectedNetwork(selectedValue);

    // Set the X-Network header dynamically
    fetch("/", {
      method: "GET",
      headers: {
        "X-Network": selectedValue,
      },
    }).catch((err) => console.error("Failed to set network header:", err));
  };

  return (
    <select
      id="network-dropdown"
      className="button button--outline"
      value={selectedNetwork}
      onChange={handleNetworkChange}
      style={{
        padding: "0.5rem 1rem",
        fontSize: "1rem",
        border: "1px solid #ccc",
        borderRadius: "5px",
        cursor: "pointer",
        backgroundColor:"#000",
        color:"var(--ifm-color)",
      }}
    >
      {networks.map((network) => (
        <option
          key={network.value}
          value={network.value}
          style={{
            fontSize: "1rem",
            padding: "0.5rem",
          }}
        >
          {network.label}
        </option>
      ))}
    </select>
  );
};

export default NetworkDropdown;
