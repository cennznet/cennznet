import { HardhatUserConfig, task } from "hardhat/config";
import "@nomicfoundation/hardhat-toolbox";
import { utils } from "ethers";

const config: HardhatUserConfig = {
  solidity: "0.8.14",
  networks: {
    hardhat: {
      chainId: 1337,
      gasPrice: utils.parseUnits("100", "gwei").toNumber(),
    },
    cennznet: {
      url: `http://localhost:9933/`,
      // Alice, Bob
      accounts: [`0xcb6df9de1efca7a3998a8ead4e02159d5fa99c3e0d4fd6432667390bb4726854`, `0x79c3b7fc0b7697b9414cb87adcb37317d1cab32818ae18c0e97ad76395d1fdcf`],
    },
  },
};

export default config;