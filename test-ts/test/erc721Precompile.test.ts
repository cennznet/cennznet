import { JsonRpcProvider, Provider } from '@ethersproject/providers';
import web3 from 'web3';
import { Api } from '@cennznet/api';
import {cvmToAddress} from "@cennznet/types/utils";
import { cryptoWaitReady, base64Encode } from '@polkadot/util-crypto';
import { expect, use } from 'chai';
import { solidity } from 'ethereum-waffle';
import {Contract, ContractFactory, Wallet} from 'ethers';
import ERC721PrecompileCaller from '../artifacts/contracts/Erc721PrecompileCaller.sol/Erc721PrecompileCaller.json';

use(solidity);

const metadataPath = {"Https": "example.com/nft/metadata" }
const name = "test-collection";
const initial_balance = 10;
const erc721Abi = [
    'event Transfer(address indexed from, address indexed to, uint256 tokenId)',
    'event Approval(address indexed owner, address indexed approved, uint256 tokenId)',
    'event ApprovalForAll(address indexed owner, address indexed operator, bool approved)',
    'function balanceOf(address who) public view returns (uint256)',
    'function ownerOf(uint256 tokenId) public view returns (address)',
    'function safeTransferFrom(address from, address to, uint256 tokenId)',
    'function transferFrom(address from, address to, uint256 tokenId)',
    'function approve(address to, uint256 tokenId)',
    'function getApproved(uint256 tokenId) public view returns (address)',
    'function setApprovalForAll(address operator, bool _approved)',
    'function isApprovedForAll(address owner, address operator) public view returns (bool)',
    'function safeTransferFrom(address from, address to, uint256 tokenId, bytes data)',
    'function name() public view returns (string memory)',
    'function symbol() public view returns (string memory)',
    'function tokenURI(uint256 tokenId) public view returns (string memory)',
];

describe('NFT precompiles', () => {
    let api: Api;
    let cennznetSigner: Wallet;
    let cennznetProvider: Provider;
    let precompileCaller: Contract;
    let nftContract: Contract;
    let globalCollectionId: any;
    let nftPrecompileAddress: string;

    before(async () => {
        const providerUri = 'http://localhost:9933';
        const wsProvider = 'ws://localhost:9944';
        cennznetProvider = new JsonRpcProvider(providerUri);
        cennznetSigner = new Wallet('0xab3fdbe7eca16f38e9f5ee81b9e23d01ef251ba4ae19225783e5921a4a8c5564').connect(cennznetProvider); // 'development' seed

        // Connect to CENNZnet local node
        await cryptoWaitReady();
        api = await Api.create({provider: wsProvider});

        // createCollection with Ethereum Address
        const cennznetAddress = cvmToAddress(cennznetSigner.address);
        let nonce = await api.rpc.system.accountNextIndex(cennznetAddress);
        let call = api.tx.nft.createCollection(name, null);
        let payload = api.registry.createType('EthWalletCall', { call: call, nonce }).toHex();
        let encodedPayload = `data:application/octet-stream;base64,${base64Encode(payload)}`;
        let signature = await cennznetSigner.signMessage(encodedPayload);
        // Broadcast the tx to CENNZnet
        await new Promise<void>((resolve) => {
            api.tx.ethWallet.call(call, cennznetSigner.address, signature).send(async ({status, events}) => {
                if (status && status.isInBlock) {
                    events.forEach(({event: {data, method}}) => {
                        if (method == 'CreateCollection') {
                            globalCollectionId = (data.toJSON() as any)[0];
                            const collection_id_hex = (+globalCollectionId).toString(16).padStart(8, '0');
                            // Assume 0 series id for first series in collection
                            const series_id_hex = (+0).toString(16).padStart(8, '0');
                            nftPrecompileAddress = web3.utils.toChecksumAddress(`0xAAAAAAAA${collection_id_hex}${series_id_hex}0000000000000000`);
                            nftContract = new Contract(nftPrecompileAddress, erc721Abi, cennznetSigner);
                            resolve();
                        }
                    });
                }
            });
        });

        // mintSeries with Ethereum Address
        nonce = await api.rpc.system.accountNextIndex(cennznetAddress);
        call = api.tx.nft.mintSeries(globalCollectionId, initial_balance, null, metadataPath, null);
        payload = api.registry.createType('EthWalletCall', { call: call, nonce }).toHex();
        encodedPayload = `data:application/octet-stream;base64,${base64Encode(payload)}`;
        signature = await cennznetSigner.signMessage(encodedPayload);
        // Broadcast the tx to CENNZnet
        await new Promise<void>((resolve) => {
            api.tx.ethWallet.call(call, cennznetSigner.address, signature).send(async ({status, events}) => {
                if (status && status.isInBlock) {
                    resolve();
                }
            });
        });

        // set series name with Ethereum Address
        nonce = await api.rpc.system.accountNextIndex(cennznetAddress);
        call = api.tx.nft.setSeriesName(globalCollectionId, 0, name);
        payload = api.registry.createType('EthWalletCall', { call: call, nonce }).toHex();
        encodedPayload = `data:application/octet-stream;base64,${base64Encode(payload)}`;
        signature = await cennznetSigner.signMessage(encodedPayload);
        // Broadcast the tx to CENNZnet
        await new Promise<void>((resolve) => {
            api.tx.ethWallet.call(call, cennznetSigner.address, signature).send(async ({status, events}) => {
                if (status && status.isInBlock) {
                    resolve();
                }
            });
        });
    });

    it('name, symbol, ownerOf, tokenURI, balanceOf', async () => {
        expect(
            await nftContract.name()
        ).to.equal(name);

        expect(
            await nftContract.symbol()
        ).to.equal(name);

        expect(
            await nftContract.ownerOf(1)
        ).to.equal(cennznetSigner.address);

        expect(
            await nftContract.balanceOf(cennznetSigner.address)
        ).to.equal(initial_balance);

        expect(
            await nftContract.tokenURI(1)
        ).to.equal("https://example.com/nft/metadata/1.json");
    })

    it('transferFrom owner', async () => {
        const receiverAddress = await Wallet.createRandom().getAddress();
        const serial_number = 0;

        // Transfer serial_number 0 to receiverAddress
        const transfer = await nftContract.connect(cennznetSigner).transferFrom(cennznetSigner.address, receiverAddress, serial_number)
        await transfer.wait();

        // Receiver_address now owner of serial_number 1
        expect(
            await nftContract.ownerOf(serial_number)
        ).to.equal(receiverAddress);
        expect(
            await nftContract.balanceOf(receiverAddress)
        ).to.equal(1);
    })

    it('approve and transferFrom via EVM', async () => {
        // Deploy test contract
        let factory = new ContractFactory(ERC721PrecompileCaller.abi, ERC721PrecompileCaller.bytecode, cennznetSigner);
        precompileCaller = await factory.deploy();
        const receiverAddress = await Wallet.createRandom().getAddress();
        const serial_number = 3;

        // Approve contract address for transfer of serial_number
        await nftContract.approve(precompileCaller.address, serial_number, {gasLimit: 50000})

        // Transfer serial_number to receiverAddress
        const transfer = await precompileCaller
            .connect(cennznetSigner)
            .transferFromProxy(nftPrecompileAddress, cennznetSigner.address, receiverAddress, serial_number, {gasLimit: 50000});
        await transfer.wait();

        // Check ownership of nft
        expect(
            await nftContract.balanceOf(receiverAddress)
        ).to.equal(1);
        expect(
            await nftContract.ownerOf(serial_number)
        ).to.equal(receiverAddress);
    })
});

