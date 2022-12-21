import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import {Api, SubmittableResult} from '@cennznet/api';
import { Keyring } from '@polkadot/keyring';
import { cryptoWaitReady } from '@polkadot/util-crypto';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import {Contract, ContractFactory, Wallet, utils, BigNumber, ethers} from 'ethers';
import DelegateCallExploit from '../artifacts/contracts/DelegateCallExploit.sol/DelegateCallExploit.json';

use(solidity);

function sleep(ms) {
    return new Promise(resolve => setTimeout(resolve, ms));
}
const GENESIS_ACCOUNT = "0x6Be02d1d3665660d22FF9624b7BE0551ee1Ac91b";

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

describe('GA precompiles', () => {
    let api: Api;
    let keyring: Keyring;
    let alice, bob;
    let cennznetSigner: Wallet;
    const cennznetSignerSS58 = '5EK7n4pa3FcCGoxvnJ4Qghe4xHJLJyNT6vsPWEJaSWUiTCVp';
    let bobSigner: Wallet;
    const bobSignerSS58 = '5Exs3yxYDxd5QCzDLAYYdT836fTYLPpfEaAarVqdKjJDxNHb'
    let cennznetProvider: Provider;
    let delegateCallContract: Contract;
    let cennzToken: Contract;
    const cennzTokenAddress = web3.utils.toChecksumAddress('0xCCCCCCCC00003e80000000000000000000000000');

    before(async () => {
        const providerUri = 'http://localhost:9933';
        cennznetProvider = new JsonRpcProvider(providerUri);
        cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed
        console.log(`signer address=${cennznetSigner.address}`);
        // 0x12B29179a7F858478Fde74f842126CdA5eA7AC35
        // 5EK7n4pa3FcCGoxvnJ4Qghe4xHJLJyNT6vsPWEJaSWUiTCVp

        cennzToken = new Contract(cennzTokenAddress, erc20Abi, cennznetSigner);


        // Connect to CENNZnet local node
        await cryptoWaitReady();
        api = await Api.create({provider: providerUri});
        keyring = new Keyring({ type: 'sr25519' });
        alice = keyring.addFromUri('//Alice');
        bob = keyring.addFromUri('//Bob');
        console.log(`Connected to CENNZnet network ${providerUri}`);

    });

    it('DELEGATE call for precompiles', async () => {
        const receiverAddress = await Wallet.createRandom().getAddress();
        console.log(`receiver address=${receiverAddress}`);

        const DELEGATECALL_FORDIDDEN_MESSAGE_1 = "0x08c379a0000000000000000000000000000000000000000000000000000000000000002000000000000000000000000000000000000000000000000000000000000000087472616e73666572000000000000000000000000000000000000000000000000"
        // const contractDetails = await createContract(context, "TestCallList");
        // contractProxy = contractDetails.contract;
        // await context.createBlock({ transactions: [contractDetails.rawTx] });
        //
        // proxyInterface = new ethers.utils.Interface((await getCompiled("TestCallList")).contract.abi);
        bobSigner = new Wallet('0x6ed7a0d3338ebaabe3e7828c81f582bd9bdbfd94e00eb6461d5fbd908bd38715').connect(cennznetProvider); // 'development' seed
        console.log(`bob signer address=${bobSigner.address}`);

        let factory = new ContractFactory(DelegateCallExploit.abi, DelegateCallExploit.bytecode, cennznetSigner);
        delegateCallContract = await factory.deploy(cennzTokenAddress, receiverAddress);
        // console.log('contract.deployTransaction:::',delegateCallContract.deployTransaction);
        await delegateCallContract.deployTransaction.wait();

        // console.log(`contract address=${delegateCallContract.address}`);
        const codeProvider = await cennznetProvider.getCode(delegateCallContract.address);
        // console.log('get code from provider....',codeProvider);






        let test = await delegateCallContract.getState();
        console.log("Value of test string: ", test);


        // Get balance of CENNZ before trap is called
        const balance_before = await api.query.genericAsset.freeBalance(16000, cennznetSignerSS58);
        console.log("balance before: ", balance_before.toString());


        const fees = await cennznetProvider.getFeeData();
        console.log(`fees: ${JSON.stringify(fees)}`);
        const nonce = await cennznetSigner.getTransactionCount();
        const iface = new utils.Interface(DelegateCallExploit.abi);
        const transferInput = iface.encodeFunctionData("trap", []);
        const maxFeePerGas = 30_001_500_000_0000; // 30_001_500_000_000 = '0x1b4944c00f00'

        const unsignedTx = {
            // eip1559 tx
            type: 2,
            from: cennznetSigner.address,
            to: delegateCallContract.address,
            nonce,
            data: "",
            gasLimit: 300_000,
            maxFeePerGas: maxFeePerGas,
            maxPriorityFeePerGas: 1500000001,
            chainId: 2999,
        };
        const signedTx = await cennznetSigner.signTransaction(unsignedTx);
        const tx = await cennznetProvider.sendTransaction(signedTx);
        const receipt = await tx.wait();


        console.log(`Receipt: ${JSON.stringify(receipt)}`);

        test = await delegateCallContract.getState();
        console.log(" ===== Value of test string after: ", test);



        // const actualGasEstimate = await delegateCallContract.connect(cennznetSigner).estimateGas.trap();
        // console.log(`actualGasEstimate = ${actualGasEstimate}`);

        // let approvedAmount = 12345;
        // await expect(
        //     cennzToken.approve(precompileCaller.address, approvedAmount)
        // ).to.emit(cennzToken, 'Approval').withArgs(cennznetSigner.address, precompileCaller.address, approvedAmount);
        //
        //
        // const tx = await delegateCallContract.connect(cennznetSigner).trap({ gasLimit: 30000 });
        // const res = await tx.wait().catch((e) => e);
        // console.log(`res = ${res}`);
        // const tx = await delegateCallContract.connect(cennznetSigner).transferProxy(receiverAddress, 69, { gasLimit: 50000 });
        // const res = await tx.wait().catch((e) => e);
        // console.log(`res = ${res}`);



        // const tx_call = await customWeb3Request(context.web3, "eth_call", [
        //     {
        //         from: GENESIS_ACCOUNT,
        //         to: delegateCallContract.options.address,
        //         gas: "0x100000",
        //         value: "0x00",
        //         data: proxyInterface.encodeFunctionData("delegateCall", [cennzTokenAddress, "0x00"]),
        //     },
        // ]);
        // const proxyInterface = new ethers.utils.Interface(
        //     DelegateCallExploit.abi
        // );

        // (cennznetProvider as any).send(
        //     {
        //         jsonrpc: "2.0",
        //         id: 1,
        //         method: "eth_call",
        //         params: [{
        //             from: GENESIS_ACCOUNT,
        //             to: delegateCallContract.address,
        //             gas: "0x100000",
        //             value: "0x00",
        //             data: proxyInterface.encodeFunctionData("trap", []),
        //         }]
        //     },
        //     (err, res) => {
        //         console.log(`res = ${res}`);
        //         console.log(`err = ${err}`);
        //     }
        // );




        // Get balance of CENNZ after trap is called
        const balance_after = await api.query.genericAsset.freeBalance(16000, cennznetSignerSS58);
        console.log("balance after: ", balance_after.toString());
        // expect(balance_before).to.eq(balance_after);



        // console.log("res=", res);
        // const error = await (await delegateCallContract.trap({ gasLimit: 500000 })).wait().then((res) => {
        //     console.log("res=", res);
        // });
        // console.log("err=", error);
        // // See expected behavior for gasLimit === 0 https://github.com/futureversecom/frontier/blob/polkadot-v0.9.27-TRN/ts-tests/tests/test-transaction-cost.ts
        // expect(error.code).to.be.eq("CALL_EXCEPTION");
        // const body = JSON.parse(error.body);
        // expect(body.error.message).to.be.eq(
        //     "cannot be called with DELEGATECALL or CALLCODE",
        // );
        // expect(error.reason).to.be.eq("processing response error");

        // await expect(delegateCallContract.trap({ gasLimit: 500000 })).to.be.revertedWith("cannot be called with DELEGATECALL or CALLCODE");


        // expect(await delegateCallContract.trap({ gasLimit: 50000 })).to.equal(DELEGATECALL_FORDIDDEN_MESSAGE);

        // let tx = await delegateCallContract.trap();

        // await expect(
        //     delegateCallContract.trap().result
        // ).to.equal(DELEGATECALL_FORDIDDEN_MESSAGE);

    }).timeout(1200000000000);
});