require('@nomiclabs/hardhat-ethers');

module.exports = {
  solidity: {
    version: "0.8.4",
      settings: {
      optimizer: {
        enabled: true,
        runs: 200
      }
    }
  }
}
