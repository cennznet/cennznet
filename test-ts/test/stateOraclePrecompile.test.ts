import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import { use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Contract, ContractFactory, Wallet, utils, BigNumber } from 'ethers';
import StateOracleDemo from '../artifacts/contracts/StateOracleDemo.sol/StateOracleDemo.json';

use(solidity);

describe('State oracle precompile', () => {
  let cennznetSigner: Wallet;
  let cennznetProvider: Provider;
  let stateOracleDemo: Contract;

  before(async () => {
    cennznetProvider = new JsonRpcProvider('http://localhost:9933');
    cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
    console.log(`signer address=${cennznetSigner.address}`);

    let factory = new ContractFactory(StateOracleDemo.abi, StateOracleDemo.bytecode, cennznetSigner);
    stateOracleDemo = await factory.deploy();
    let stateOracleDemoBalance = (await cennznetProvider.getBalance(stateOracleDemo.address)).toString();
    console.log(`demo contract address: ${stateOracleDemo.address}, balance: ${stateOracleDemoBalance}`);
  });

  it('oracle request', async () => {
    stateOracleDemo.helloEthereum(
      '0x5FbDB2315678afecb367f032d93F642f64180aa3',
      '0xf39Fd6e51aad88F6F4ce6aB8827279cffFb92266',
      { value: utils.parseEther('2000.0')},
    );
  });
});