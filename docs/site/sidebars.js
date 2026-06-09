// Copyright (c) Mysten Labs, Inc.
// Modifications Copyright (c) 2024 IOTA Stiftung
// SPDX-License-Identifier: Apache-2.0

const developer = require("../content/sidebars/developer.js");
const iotaEvm = require('../content/sidebars/iota-evm.js');
const aboutIota = require("../content/sidebars/about-iota.js");
const operator = require("../content/sidebars/operator.js");
const users = require("../content/sidebars/users.js");


const sidebars = {
  developerSidebar: developer,
  iotaEvmSidebar: iotaEvm,
  operatorSidebar: operator,
  aboutIotaSidebar: aboutIota,
  usersSidebar: users,
};

module.exports = sidebars;
