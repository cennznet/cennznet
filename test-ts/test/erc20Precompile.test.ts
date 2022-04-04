import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Contract, ContractFactory, Wallet, utils, BigNumber } from 'ethers';
import PrecompileCaller from '../artifacts/contracts/PrecompileCaller.sol/PrecompileCaller.json';

use(solidity);

describe('GA precompiles', () => {
  let cennznetSigner: Wallet;
  let cennznetProvider: Provider;
  let precompileCaller: Contract;
  let cennzToken: Contract;
  const cennzTokenAddress = web3.utils.toChecksumAddress('0xCCCCCCCC00003e80000000000000000000000000');
  const erc20Abi = [
    'event Transfer(address indexed from, address indexed to, uint256 value)',
    'event Approval(address indexed owner, address indexed spender, uint256 value)',
    'function approve(address spender, uint256 amount) public returns (bool)',
    'function allowance(address owner, address spender) public view returns (uint256)',
    'function balanceOf(address who) public view returns (uint256)',
    'function name() public view returns (string memory)',
    'function symbol() public view returns (string memory)',
    'function decimals() public view returns (uint8)',
    'function transfer(address who, uint256 amount)',
  ];

  before(async () => {
    cennznetProvider = new JsonRpcProvider('http://localhost:9933');
    cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
    cennzToken = new Contract(cennzTokenAddress, erc20Abi, cennznetSigner);
    console.log(`signer address=${cennznetSigner.address}`);
  });

  it('name, symbol, decimals', async () => {
    expect(
      await cennzToken.decimals()
    ).to.equal(4);

    expect(
      await cennzToken.name()
    ).to.equal("CENNZ");

    expect(
      await cennzToken.symbol()
    ).to.equal("CENNZ");
  });

  it('approve and transferFrom', async () => {
    let walletCENNZ = await cennzToken.balanceOf(cennznetSigner.address);
    console.log(`dev account CENNZ=${walletCENNZ.toString()}`);
    console.log(`dev account CPAY=${(await cennznetSigner.getBalance())}`);

    let factory = new ContractFactory(PrecompileCaller.abi, PrecompileCaller.bytecode, cennznetSigner);
    precompileCaller = await factory.deploy();
    console.log(`contract address=${precompileCaller.address}`);

    let approvedAmount = 12345;
    await expect(
      cennzToken.approve(precompileCaller.address, approvedAmount)
    ).to.emit(cennzToken, 'Approval').withArgs(cennznetSigner.address, precompileCaller.address, approvedAmount);

    expect(
      await cennzToken.allowance(cennznetSigner.address, precompileCaller.address)
    ).to.equal(approvedAmount);

    await expect(
      precompileCaller.takeCENNZ(approvedAmount)
    ).to.emit(cennzToken, 'Transfer').withArgs(cennznetSigner.address, precompileCaller.address, approvedAmount);

  }).timeout(15000);

  it('CENNZ transfer', async () => {
    await expect(
      cennzToken.transfer(cennznetSigner.address, 555)
    ).to.emit(cennzToken, 'Transfer').withArgs(cennznetSigner.address, cennznetSigner.address, 555);
  }).timeout(15000);

  it('CPAY transfer amounts via EVM', async () => {
    // fund the contract with some CPAY
    let endowment = utils.parseEther('6');
    let endowTx = await cennznetSigner.sendTransaction(
      {
        to: precompileCaller.address,
        value: endowment,
        gasLimit: 500000,
      }
    );
    await endowTx.wait();
    expect(await cennznetProvider.getBalance(precompileCaller.address)).to.be.equal(endowment);
    console.log('endowed 6 CPAY');

    const receiverAddress = await Wallet.createRandom().getAddress();
    let tx = await precompileCaller.sendCPAYAmounts(receiverAddress);
    await tx.wait();
  }).timeout(12000);

  it('CPAY transfer amounts via transaction', async () => {
    const receiverAddress = await Wallet.createRandom().getAddress();
    // pairs of (input amount, actual transferred amount)
    // shows the behaviour of the CPAY scaling rules
    let payments = [
      // transfer smallest unit of cpay
      [utils.parseEther('0.0001'), utils.parseEther('0.0001')],
      // transfer 1.2345 cpay
      [utils.parseEther('1.2345'), utils.parseEther('1.2345')],
      // transfer < the smallest unit of cpay 0.0001, rounds up
      [utils.parseEther('0.000099'), utils.parseEther('0.0001')],
      // transfer amounts with some part < the smallest unit of cpay
      [utils.parseEther('1.00005'), utils.parseEther('1.0001')],
      [utils.parseEther('1.000050000001'), utils.parseEther('1.0001')],
      [utils.parseEther('1.00009999'), utils.parseEther('1.0001')],
    ];
    let total: BigNumber = BigNumber.from(0);
    for (const [payment, expected] of payments) {
      let tx = await cennznetSigner.sendTransaction(
        {
          to: receiverAddress,
          value: payment,
        }
      );
      await tx.wait();
      let balance = await cennznetProvider.getBalance(receiverAddress);
      total = total.add(expected);
      console.log(`input:       ${payment.toString()}\nreal:        ${expected.toString()}\nnew expected:${total.toString()}\nnew balance: ${balance}\n`);
      expect(balance).to.be.equal(total.toString());

      // sleep, prevents nonce issues
      await new Promise(r => setTimeout(r, 500));
    }
  }).timeout(60 * 1000);
});