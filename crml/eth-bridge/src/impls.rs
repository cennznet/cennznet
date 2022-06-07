/* Copyright 2021-2022 Centrality Investments Limited
*
* Licensed under the LGPL, Version 3.0 (the "License");
* you may not use this file except in compliance with the License.
* Unless required by applicable law or agreed to in writing, software
* distributed under the License is distributed on an "AS IS" BASIS,
* WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
* See the License for the specific language governing permissions and
* limitations under the License.
* You may obtain a copy of the License at the root of this project source code,
* or at:
*     https://centrality.ai/licenses/gplv3.txt
*     https://centrality.ai/licenses/lgplv3.txt
*/
use crate::{
	log,
	rt_offchain::{http::Request, Duration},
	types::{
		BridgeEthereumRpcApi, BridgeRpcError, Bytes, EthAddress, EthBlock, EthCallRpcRequest, EthHash, EthResponse,
		GetBlockRequest, GetTxReceiptRequest, LatestOrNumber, TransactionReceipt,
	},
};
use sp_runtime::offchain::StorageKind;
use sp_std::{convert::TryInto, prelude::*};

#[cfg(not(feature = "std"))]
use sp_std::alloc::string::ToString;
#[cfg(std)]
use std::string::ToString;

/// Deadline for any network requests e.g.to Eth JSON-RPC endpoint
/// Allows ~3 offchain requests per block
const REQUEST_TTL_MS: u64 = 1_500;

/// Provides minimal ethereum RPC queries for eth bridge protocol
pub struct EthereumRpcClient;

impl BridgeEthereumRpcApi for EthereumRpcClient {
	/// Issue an `eth_call` request to `target` address with `input`
	/// Returns the abi encoded 'returndata'
	fn eth_call(target: EthAddress, input: &[u8], at_block: LatestOrNumber) -> Result<Vec<u8>, BridgeRpcError> {
		let request = EthCallRpcRequest::new(target, input, random_request_id(), at_block);
		let resp_bytes = Self::query_eth_client(Some(request))?;

		// Deserialize JSON to struct
		serde_json::from_slice::<EthResponse<Bytes>>(&resp_bytes)
			.map(|resp| resp.result.map(|b| b.0).unwrap_or_default())
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				BridgeRpcError::InvalidJSON
			})
	}

	/// Get latest block number from eth client
	fn get_block_by_number(req: LatestOrNumber) -> Result<Option<EthBlock>, BridgeRpcError> {
		// TODO: #670 add a block cache
		let request = match req {
			LatestOrNumber::Latest => GetBlockRequest::latest(1_usize),
			LatestOrNumber::Number(n) => GetBlockRequest::for_number(1_usize, n),
		};
		let resp_bytes = Self::query_eth_client(request).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			BridgeRpcError::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_slice::<EthResponse<EthBlock>>(&resp_bytes)
			.map(|resp| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				BridgeRpcError::InvalidJSON
			})
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: EthHash) -> Result<Option<TransactionReceipt>, BridgeRpcError> {
		let request = GetTxReceiptRequest::new(tx_hash, random_request_id());
		let resp_bytes = Self::query_eth_client(Some(request)).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			BridgeRpcError::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_slice::<EthResponse<TransactionReceipt>>(&resp_bytes)
			.map(|resp| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				BridgeRpcError::InvalidJSON
			})
	}
}

impl EthereumRpcClient {
	/// This function uses the `offchain::http` API to query the remote ethereum information,
	/// and returns the JSON response as vector of bytes.
	fn query_eth_client<R: serde::Serialize>(request_body: R) -> Result<Vec<u8>, BridgeRpcError> {
		// Load eth http URI from offchain storage
		// this should have been configured on start up by passing e.g. `--eth-http`
		// e.g. `--eth-http=http://localhost:8545`
		let eth_http_uri = if let Some(value) = sp_io::offchain::local_storage_get(StorageKind::PERSISTENT, b"ETH_HTTP")
		{
			value
		} else {
			log!(
				error,
				"💎 Eth http uri is not configured! set --eth-http=<value> on start up"
			);
			return Err(BridgeRpcError::OcwConfig);
		};
		let eth_http_uri = core::str::from_utf8(&eth_http_uri).map_err(|_| BridgeRpcError::OcwConfig)?;

		const HEADER_CONTENT_TYPE: &str = "application/json";
		log!(info, "💎 sending request to: {}", eth_http_uri);
		let body = serde_json::to_string::<R>(&request_body).unwrap();
		let body_raw = body.as_bytes();
		// Initiate an external HTTP POST request. This is using high-level wrappers from `sp_runtime`.
		let request = Request::post(eth_http_uri, vec![body_raw]);
		log!(trace, "💎 request: {:?}", request);

		// Keeping the offchain worker execution time reasonable, so limiting the call to be within 3s.
		let timeout = sp_io::offchain::timestamp().add(Duration::from_millis(REQUEST_TTL_MS));
		let pending = request
			.add_header("Content-Type", HEADER_CONTENT_TYPE)
			.add_header("Content-Length", &body_raw.len().to_string())
			.deadline(timeout) // Setting the timeout time
			.send() // Sending the request out by the host
			.map_err(|err| {
				log!(error, "💎 http request error: {:?}", err);
				BridgeRpcError::HttpFetch
			})?;

		// By default, the http request is async from the runtime perspective. So we are asking the
		// runtime to wait here.
		// The returning value here is a `Result` of `Result`, so we are unwrapping it twice by two `?`
		// ref: https://substrate.dev/rustdocs/v3.0.0/sp_runtime/offchain/http/struct.PendingRequest.html#method.try_wait
		let response = pending
			.try_wait(timeout)
			.map_err(|err| {
				log!(error, "💎 http request error: timeline reached: {:?}", err);
				BridgeRpcError::HttpFetch
			})?
			.map_err(|err| {
				log!(error, "💎 http request error: timeline reached: {:?}", err);
				BridgeRpcError::HttpFetch
			})?;
		log!(trace, "💎 response: {:?}", response);

		if response.code != 200 {
			log!(error, "💎 http request status code: {}", response.code);
			return Err(BridgeRpcError::HttpFetch);
		}

		// Read the response body and check it's valid utf-8
		Ok(response.body().collect::<Vec<u8>>())
	}
}

/// Return a random usize value
fn random_request_id() -> usize {
	u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap()) as usize
}

#[cfg(test)]
mod tests {
	use super::*;
	use parking_lot::RwLock;
	use sp_core::offchain::{
		testing::{OffchainState, PendingRequest, TestOffchainExt},
		OffchainDbExt, OffchainWorkerExt,
	};
	use sp_io::TestExternalities;
	use sp_std::sync::Arc;

	/// a fake URI to use as the configured `--eth-http` endpoint
	const MOCK_TEST_ENDPOINT: &'static str = "http://example.com";

	/// Build `PendingRequest`s
	struct PendingRequestBuilder(PendingRequest);

	impl PendingRequestBuilder {
		fn new() -> Self {
			Self {
				0: PendingRequest {
					uri: MOCK_TEST_ENDPOINT.into(),
					sent: true,
					..Default::default()
				},
			}
		}
		fn request(mut self, request: &[u8]) -> Self {
			self.0.body = request.to_vec();
			self.0.headers = vec![
				("Content-Type".to_string(), "application/json".to_string()),
				("Content-Length".to_string(), request.len().to_string()),
			];
			self
		}
		fn method(mut self, method: &str) -> Self {
			self.0.method = method.into();
			self
		}
		fn response(mut self, response: &[u8]) -> Self {
			self.0.response = Some(response.to_vec());
			self
		}
		fn build(self) -> PendingRequest {
			self.0
		}
	}

	/// Setup mock offchain environment suitable for testing http requests
	fn mock_offchain_env() -> (TestExternalities, Arc<RwLock<OffchainState>>) {
		let (offchain, state) = TestOffchainExt::new();
		let mut t = sp_io::TestExternalities::default();
		t.register_extension(OffchainDbExt::new(offchain.clone()));
		t.register_extension(OffchainWorkerExt::new(offchain));
		// setup ---eth-http uri
		t.execute_with(|| {
			sp_io::offchain::local_storage_compare_and_set(
				StorageKind::PERSISTENT,
				b"ETH_HTTP",
				None,
				MOCK_TEST_ENDPOINT.as_bytes(),
			);
		});

		(t, state)
	}

	#[test]
	fn eth_call() {
		let (mut ext, state) = mock_offchain_env();
		// define the expected JSON-RPC payload for the eth_call request and a mock response
		{
			let expected_request = br#"{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x0000000000000000000000000000000000000002","data":"0x0102030405"},"latest"],"id":0}"#;
			let mock_response = br#"{"jsonrpc":"2.0","id":0,"result":"0x050403"}"#;
			let expected_request_response = PendingRequestBuilder::new()
				.method("POST")
				.request(expected_request)
				.response(mock_response)
				.build();
			state.write().expect_request(expected_request_response);
		}

		// test
		ext.execute_with(|| {
			assert_eq!(
				EthereumRpcClient::eth_call(
					EthAddress::from_low_u64_be(2_u64), // 0x0000000000000000000000000000000000000002
					&[1_u8, 2, 3, 4, 5],                // 0x0102030405
					LatestOrNumber::Latest,
				),
				// return data
				Ok(vec![5_u8, 4, 3]), // 0x050403
			);
		})
	}

	#[test]
	fn eth_call_at_block_empty_response() {
		let (mut ext, state) = mock_offchain_env();
		// define the expected JSON-RPC payload for the eth_call request and a mock response
		{
			let expected_request = br#"{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x0000000000000000000000000000000000000002","data":"0x0102030405"},"0xff"],"id":0}"#;
			let mock_response = br#"{"jsonrpc":"2.0","id":0,"result":"0x"}"#;
			let expected_request_response = PendingRequestBuilder::new()
				.method("POST")
				.request(expected_request)
				.response(mock_response)
				.build();
			state.write().expect_request(expected_request_response);
		}

		// test
		ext.execute_with(|| {
			assert_eq!(
				EthereumRpcClient::eth_call(
					EthAddress::from_low_u64_be(2_u64),
					&[1_u8, 2, 3, 4, 5],
					LatestOrNumber::Number(0xff),
				),
				Ok(vec![]),
			);
		})
	}

	#[test]
	fn eth_call_at_zero_address_empty_input() {
		let (mut ext, state) = mock_offchain_env();
		// define the expected JSON-RPC payload for the eth_call request and a mock response
		{
			let expected_request = br#"{"jsonrpc":"2.0","method":"eth_call","params":[{"to":"0x0000000000000000000000000000000000000000","data":"0x"},"latest"],"id":0}"#;
			let mock_response = br#"{"jsonrpc":"2.0","id":0,"result":"0x"}"#;
			let expected_request_response = PendingRequestBuilder::new()
				.method("POST")
				.request(expected_request)
				.response(mock_response)
				.build();
			state.write().expect_request(expected_request_response);
		}

		// test
		ext.execute_with(|| {
			assert_eq!(
				EthereumRpcClient::eth_call(EthAddress::zero(), Default::default(), LatestOrNumber::Latest,),
				Ok(vec![]),
			);
		})
	}
}
