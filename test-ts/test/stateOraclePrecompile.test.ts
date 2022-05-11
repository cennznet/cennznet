import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Contract, ContractFactory, Wallet, utils, BigNumber } from 'ethers';
import StateOracleDemo from '../artifacts/contracts/StateOracleDemo.sol/StateOracleDemo.json';

use(solidity);

describe('State oracle precompile', () => {
  let cennznetSigner: Wallet;
  let cennznetProvider: Provider;
  let stateOraclePrecompile: Contract;
  let stateOracleDemo: Contract;
  const stateOracleAddress = web3.utils.toChecksumAddress('0xCCCCCCCC00003e80000000000000000000000000');
  const stateOracleAbi = [
    "function remoteCall(address target, bytes input, bytes4 callbackSelector, uint256 callbackGasLimit, uint256 callbackBounty)",
  ];

  before(async () => {
    cennznetProvider = new JsonRpcProvider('http://localhost:9933');
    cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
    stateOraclePrecompile = new Contract(stateOracleAddress, stateOracleAbi, cennznetSigner);
    console.log(`signer address=${cennznetSigner.address}`);

    let factory = new ContractFactory(StateOracleDemo.abi, StateOracleDemo.bytecode, cennznetSigner);
    stateOracleDemo = await factory.deploy();
  });

  it('remoteCall request', async () => {
    stateOracleDemo.helloEthereum('0x5FbDB2315678afecb367f032d93F642f64180aa3', '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266');
  });
});