require('@nomiclabs/hardhat-ethers');
require("dotenv").config();

module.exports = {
  solidity: {
    version: "0.8.17",
      settings: {
      optimizer: {
        enabled: true,
        runs: 200
      }
    }
  },
  networks: {
    cennznet: {
      url: `http://localhost:9933/`,
      accounts: [`0xcb6df9de1efca7a3998a8ead4e02159d5fa99c3e0d4fd6432667390bb4726854`],
    },
  },
}