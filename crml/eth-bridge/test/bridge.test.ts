import { expect, use } from 'chai';
import { Contract } from 'ethers';
import { deployContract, MockProvider, solidity } from 'ethereum-waffle';
import CENNZnetBridge from '../artifacts/contracts/CENNZnetBridge.sol/CENNZnetBridge.json';
import TestToken from '../artifacts/contracts/TestToken.sol/TestToken.json';

use(solidity);

describe('CENNZnetBridge', () => {
  const [wallet, walletTo] = new MockProvider().getWallets();
  let bridge: Contract;
  let testToken: Contract;

  beforeEach(async () => {
    testToken = await deployContract(wallet, TestToken, [1000000]);
    bridge = await deployContract(wallet, CENNZnetBridge, []);
  });

  it('deposits disabled on init', async () => {
    expect(!await bridge.isBridgeActive)
  });

  it('deposit active/pause', async () => {
    await bridge.activateDeposits();
    expect(await bridge.isBridgeActive)

    await bridge.pauseDeposits();
    expect(!await bridge.isBridgeActive)
  });

  it('deposit success', async () => {
    let depositAmount = 7;
    let cennznetAddress = "0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10";
    await bridge.activateDeposits();
    await testToken.approve(bridge.address, depositAmount);

    // best-effort guess at block timestamp
    let timestamp = Math.floor(new Date().getTime() / 1000);

    await expect(
      bridge.deposit(testToken.address, depositAmount, cennznetAddress)
    ).to.emit(bridge, 'Deposit').withArgs(wallet.address, testToken.address, depositAmount, cennznetAddress, timestamp);

    expect(await bridge.balances(wallet.address, testToken.address)).to.equal(depositAmount);
  });

  it('deposit, bridge inactive', async () => {
    await testToken.approve(bridge.address, 7);

    await expect(
      bridge.deposit(testToken.address, 7, "0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10")
    ).to.revertedWith('deposits paused');
  });

  it('deposit, no approval', async () => {
    await bridge.activateDeposits();

    await expect(
      bridge.deposit(testToken.address, 7, "0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10")
    ).to.be.reverted;
  });

  it('deposit, zero amount', async () => {
    await bridge.activateDeposits();

    await expect(
      bridge.deposit(testToken.address, 0, "0xacd6118e217e552ba801f7aa8a934ea6a300a5b394e7c3f42cd9d6dd9a457c10")
    ).to.revertedWith('no tokens deposited');
  });

});
