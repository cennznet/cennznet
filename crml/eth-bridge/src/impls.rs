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
		BridgeEthereumRpcApi, BridgeRpcError, EthAddress, EthBlock, EthCallRpcRequest, EthHash, EthResponse,
		GetBlockRequest, GetTxReceiptRequest, LatestOrNumber, TransactionReceipt,
	},
	REQUEST_TTL_MS,
};
use sp_runtime::offchain::StorageKind;
use sp_std::{convert::TryInto, prelude::*};

#[cfg(not(feature = "std"))]
use sp_std::alloc::string::ToString;
#[cfg(std)]
use std::string::ToString;

/// Provides minimal ethereum RPC queries for eth bridge protocol
pub struct EthereumRpcClient;

impl BridgeEthereumRpcApi for EthereumRpcClient {
	/// Get latest block number from eth client
	fn get_block_by_number(req: LatestOrNumber) -> Result<Option<EthBlock>, BridgeRpcError> {
		let request = match req {
			LatestOrNumber::Latest => GetBlockRequest::latest(1_usize),
			LatestOrNumber::Number(n) => GetBlockRequest::for_number(1_usize, n),
		};
		let resp_bytes = Self::query_eth_client(request).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			BridgeRpcError::HttpFetch
		})?;

		let resp_str = core::str::from_utf8(&resp_bytes).map_err(|_| {
			log!(error, "💎 response invalid utf8: {:?}", resp_bytes);
			BridgeRpcError::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_str::<EthResponse<EthBlock>>(resp_str)
			.map(|resp| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				BridgeRpcError::HttpFetch
			})
	}

	/// Get transaction receipt from eth client
	fn get_transaction_receipt(tx_hash: EthHash) -> Result<Option<TransactionReceipt>, BridgeRpcError> {
		let random_request_id = u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap());
		let request = GetTxReceiptRequest::new(tx_hash, random_request_id as usize);
		let resp_bytes = Self::query_eth_client(Some(request)).map_err(|e| {
			log!(error, "💎 read eth-rpc API error: {:?}", e);
			BridgeRpcError::HttpFetch
		})?;

		let resp_str = core::str::from_utf8(&resp_bytes).map_err(|_| {
			log!(error, "💎 response invalid utf8: {:?}", resp_bytes);
			BridgeRpcError::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_str::<EthResponse<TransactionReceipt>>(resp_str)
			.map(|resp| resp.result)
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				BridgeRpcError::HttpFetch
			})
	}

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
		// TODO Log request
		// log!(trace, "💎 request: {:?}", request);

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

	/// Issue an `eth_call` request to `target` address with `input`
	/// Returns the abi encoded 'returndata'
	fn eth_call(target: EthAddress, input: &[u8], at_block: LatestOrNumber) -> Result<Vec<u8>, BridgeRpcError> {
		let random_request_id = u32::from_be_bytes(sp_io::offchain::random_seed()[..4].try_into().unwrap());
		let request = EthCallRpcRequest::new(target, input, random_request_id as usize, at_block);
		let resp_bytes = Self::query_eth_client(Some(request))?;

		let resp_str = core::str::from_utf8(&resp_bytes).map_err(|_| {
			log!(error, "💎 response invalid utf8: {:?}", resp_bytes);
			BridgeRpcError::HttpFetch
		})?;

		// Deserialize JSON to struct
		serde_json::from_str::<EthResponse<Vec<u8>>>(&resp_str)
			.map(|resp| resp.result.unwrap_or_default())
			.map_err(|err| {
				log!(error, "💎 deserialize json response error: {:?}", err);
				BridgeRpcError::HttpFetch
			})
	}
}

#[cfg(test)]
mod tests {
	use super::*;
	use sp_core::offchain::{testing, OffchainDbExt, OffchainWorkerExt};

	#[test]
	fn eth_call() {
		let (offchain, state) = testing::TestOffchainExt::new();
		let mut t = sp_io::TestExternalities::default();
		t.register_extension(OffchainDbExt::new(offchain.clone()));
		t.register_extension(OffchainWorkerExt::new(offchain));

		// setup ---eth-http uri
		t.execute_with(|| {
			sp_io::offchain::local_storage_compare_and_set(
				StorageKind::PERSISTENT,
				b"ETH_HTTP",
				None,
				b"http://example.com",
			);
		});

		{
			let mut state = state.write();
			state.expect_request(testing::PendingRequest {
				method: "POST".into(),
				uri: "https://example.com".into(),
				response: Some(br#" {"id":1,"jsonrpc": "2.0","result": "0x010203"}"#.to_vec()),
				sent: true,
				..Default::default()
			});
		}

		t.execute_with(|| {
			assert_eq!(
				EthereumRpcClient::eth_call(
					EthAddress::from_low_u64_be(555_u64),
					&[1_u8, 2, 3, 4, 5],
					LatestOrNumber::Latest,
				),
				Ok(vec![1_u8, 2, 3]),
			);
		})
	}
}
