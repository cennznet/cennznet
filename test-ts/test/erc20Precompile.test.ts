import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import { Api } from '@cennznet/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import { Contract, ContractFactory, Wallet, utils, BigNumber } from 'ethers';
import PrecompileCaller from '../artifacts/contracts/Erc20PrecompileCaller.sol/Erc20PrecompileCaller.json';

use(solidity);

function sleep(ms) {
  return new Promise(resolve => setTimeout(resolve, ms));
}

describe('GA precompiles', () => {
  let api: Api;
  let keyring: Keyring;
  let alice, bob;
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

  const feeProxyAbi = [
    'function callWithFeePreferences(uint32 asset, uint128 max_payment, address target, bytes input)',
  ]

  before(async () => {
    const providerUri = 'http://localhost:9933';
    cennznetProvider = new JsonRpcProvider(providerUri);
    cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
    cennzToken = new Contract(cennzTokenAddress, erc20Abi, cennznetSigner);
    console.log(`signer address=${cennznetSigner.address}`);

    // Connect to CENNZnet local node
    await cryptoWaitReady();
    api = await Api.create({provider: providerUri});
    keyring = new Keyring({ type: 'sr25519' });
    alice = keyring.addFromUri('//Alice');
    bob = keyring.addFromUri('//Bob');
    console.log(`Connected to CENNZnet network ${providerUri}`);
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

  it('transfer with fee preferences no liquidity', async () => {
    // Fee Proxy 1211 as an address
    const feeProxyAddress = '0x00000000000000000000000000000000000004bb';
    const feeProxy = new Contract(feeProxyAddress, feeProxyAbi, cennznetSigner);

    const receiverAddress = await Wallet.createRandom().getAddress();
    const transferAmount = 12345;
    let iface = new utils.Interface(erc20Abi);
    const transferInput = iface.encodeFunctionData("transfer", [receiverAddress, transferAmount]);
    const asset = 15000;
    const max_payment = 50;

    await expect(
        feeProxy.callWithFeePreferences(asset, max_payment, cennzTokenAddress, transferInput)
    ).to.reverted;
  }).timeout(15000);

  it('transfer with fee preferences', async () => {
    // Fee Proxy 1211 as an address
    const feeProxyAddress = '0x00000000000000000000000000000000000004bb';
    const feeProxy = new Contract(feeProxyAddress, feeProxyAbi, cennznetSigner);
    const receiverAddress = await Wallet.createRandom().getAddress();
    const transferAmount = 12345;
    const asset = 16000;
    const max_payment = 50;
    const amount = 3_000_000_000;
    let iface = new utils.Interface(erc20Abi);
    const transferInput = iface.encodeFunctionData("transfer", [receiverAddress, transferAmount]);

    // Add liquidity for the swap
    await api.tx.cennzx.addLiquidity(asset, 1, amount, amount).signAndSend(alice);
    // Sleep for more than one block to ensure liquidity has been added
    await sleep(6000);

    await expect(
        feeProxy.callWithFeePreferences(asset, max_payment, cennzTokenAddress, transferInput)
    ).to.emit(cennzToken, 'Transfer').withArgs(cennznetSigner.address, receiverAddress, transferAmount);
  }).timeout(30000);

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