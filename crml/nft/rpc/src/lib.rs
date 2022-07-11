// Copyright 2020-2021 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
// This file is part of Substrate.

// Substrate is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.

// Substrate is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.

// You should have received a copy of the GNU General Public License
// along with Substrate.  If not, see <http://www.gnu.org/licenses/>.

//! Node-specific RPC methods for interaction with NFT module.

use std::sync::Arc;

use cennznet_primitives::types::{BlockNumber, CollectionId, SerialNumber, SeriesId, TokenId};
use codec::Codec;
use crml_nft::{CollectionInfo, Config, Listing, ListingResponse, ListingResponseWrapper, TokenInfo};
use jsonrpsee::{
	core::{Error as RpcError, RpcResult},
	proc_macros::rpc,
};
use sp_api::ProvideRuntimeApi;
use sp_blockchain::HeaderBackend;
use sp_runtime::{generic::BlockId, traits::Block as BlockT};

pub use crml_nft_rpc_runtime_api::{self as runtime_api, NftApi as NftRuntimeApi};

/// NFT RPC methods.
#[rpc(client, server, namespace = "nft")]
pub trait NftApi<AccountId> {
	#[method(name = "collectedTokens")]
	fn collected_tokens(&self, collection_id: CollectionId, who: AccountId) -> RpcResult<Vec<TokenId>>;

	#[method(name = "getCollectionInfo")]
	fn collection_info(&self, collection_id: CollectionId) -> RpcResult<Option<CollectionInfo<AccountId>>>;

	#[method(name = "tokenUri")]
	fn token_uri(&self, token_id: TokenId) -> RpcResult<Vec<u8>>;

	#[method(name = "getTokenInfo")]
	fn token_info(
		&self,
		collection_id: CollectionId,
		series_id: SeriesId,
		serial_number: SerialNumber,
	) -> RpcResult<Option<TokenInfo<AccountId>>>;

	#[method(name = "getCollectionListings")]
	fn collection_listings(
		&self,
		collection_id: CollectionId,
		cursor: u128,
		limit: u16,
	) -> RpcResult<ListingResponseWrapper<AccountId>>;
}

/// An implementation of NFT specific RPC methods.
pub struct Nft<C, Block, T: Config> {
	client: Arc<C>,
	_marker: std::marker::PhantomData<(Block, T)>,
}

impl<C, Block, T: Config> Nft<C, Block, T> {
	/// Create new `Nft` with the given reference to the client.
	pub fn new(client: Arc<C>) -> Self {
		Nft {
			client,
			_marker: Default::default(),
		}
	}
}

impl<C, Block, AccountId, T> NftApiServer<AccountId> for Nft<C, Block, T>
where
	Block: BlockT,
	T: Config<AccountId = AccountId, BlockNumber = BlockNumber> + Send + Sync,
	C: Send + Sync + 'static + ProvideRuntimeApi<Block> + HeaderBackend<Block>,
	C::Api: NftRuntimeApi<Block, AccountId, T>,
	AccountId: Codec,
{
	fn collected_tokens(&self, collection_id: CollectionId, who: AccountId) -> RpcResult<Vec<TokenId>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);
		api.collected_tokens(&at, collection_id, who)
			.map_err(|e| RpcError::to_call_error(e))
	}

	fn token_uri(&self, token_id: TokenId) -> RpcResult<Vec<u8>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);
		api.token_uri(&at, token_id).map_err(|e| RpcError::to_call_error(e))
	}

	fn collection_info(&self, collection_id: CollectionId) -> RpcResult<Option<CollectionInfo<AccountId>>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		api.collection_info(&at, collection_id)
			.map_err(|e| RpcError::to_call_error(e))
	}

	fn token_info(
		&self,
		collection_id: CollectionId,
		series_id: SeriesId,
		serial_number: SerialNumber,
	) -> RpcResult<Option<TokenInfo<AccountId>>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);
		api.token_info(&at, collection_id, series_id, serial_number)
			.map_err(|e| RpcError::to_call_error(e))
	}

	fn collection_listings(
		&self,
		collection_id: CollectionId,
		offset: u128,
		limit: u16,
	) -> RpcResult<ListingResponseWrapper<AccountId>> {
		let api = self.client.runtime_api();
		let best = self.client.info().best_hash;
		let at = BlockId::hash(best);

		let result = api
			.collection_listings(&at, collection_id, offset, limit)
			.map_err(|e| RpcError::to_call_error(e))?;

		let new_cursor = result.0;
		let result = result
			.1
			.into_iter()
			.map(|(listing_id, listing)| match listing {
				Listing::FixedPrice(fixed_price) => ListingResponse {
					id: listing_id,
					listing_type: "fixedPrice".as_bytes().to_vec(),
					payment_asset: fixed_price.payment_asset,
					price: fixed_price.fixed_price,
					end_block: fixed_price.close,
					buyer: fixed_price.buyer,
					seller: fixed_price.seller,
					token_ids: fixed_price.tokens,
					royalties: fixed_price.royalties_schedule.entitlements,
				},
				Listing::Auction(auction) => ListingResponse {
					id: listing_id,
					listing_type: "auction".as_bytes().to_vec(),
					payment_asset: auction.payment_asset,
					price: auction.reserve_price,
					end_block: auction.close,
					buyer: None,
					seller: auction.seller,
					token_ids: auction.tokens,
					royalties: auction.royalties_schedule.entitlements,
				},
			})
			.collect();

		Ok(ListingResponseWrapper {
			listings: result,
			new_cursor,
		})
	}
}
