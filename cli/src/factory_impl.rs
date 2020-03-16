// Copyright 2018-2020 Parity Technologies (UK) Ltd. and Centrality Investments Ltd.
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

//! Implementation of the transaction factory trait, which enables
//! using the cli to manufacture transactions and distribute them
//! to accounts.

use rand::rngs::StdRng;
use rand::{Rng, SeedableRng};

use cennznet_primitives::types::Signature;
use cennznet_runtime::{Call, CheckedExtrinsic, GenericAssetCall, MinimumPeriod, SignedExtra, UncheckedExtrinsic};
use codec::{Decode, Encode};
use node_transaction_factory::RuntimeAdapter;
use sp_core::{crypto::Pair, sr25519};
use sp_finality_tracker;
use sp_inherents::InherentData;
use sp_keyring::sr25519::Keyring;
use sp_runtime::{
	generic::Era,
	traits::{Block as BlockT, Header as HeaderT, IdentifyAccount, SignedExtension, Verify, Zero},
};
use sp_timestamp;

use cennznet_runtime::constants::asset::SPENDING_ASSET_ID;

type AccountPublic = <Signature as Verify>::Signer;

pub struct FactoryState<N> {
	blocks: u32,
	transactions: u32,
	block_number: N,
	index: u32,
}

type Number = <<cennznet_primitives::types::Block as BlockT>::Header as HeaderT>::Number;

impl<Number> FactoryState<Number> {
	fn build_extra(index: cennznet_primitives::types::Index, phase: u64) -> cennznet_runtime::SignedExtra {
		(
			None,
			frame_system::CheckVersion::new(),
			frame_system::CheckGenesis::new(),
			frame_system::CheckEra::from(Era::mortal(256, phase)),
			frame_system::CheckNonce::from(index),
			frame_system::CheckWeight::new(),
			crml_transaction_payment::ChargeTransactionPayment::from(0, None),
			Default::default(),
		)
	}
}

impl RuntimeAdapter for FactoryState<Number> {
	type AccountId = cennznet_primitives::types::AccountId;
	type Balance = cennznet_primitives::types::Balance;
	type Block = cennznet_primitives::types::Block;
	type Phase = sp_runtime::generic::Phase;
	type Secret = sr25519::Pair;
	type Index = cennznet_primitives::types::Index;

	type Number = Number;

	fn new(blocks: u32, transactions: u32) -> FactoryState<Self::Number> {
		FactoryState {
			blocks,
			transactions,
			block_number: 0,
			index: 0,
		}
	}

	fn block_number(&self) -> u32 {
		self.block_number
	}

	fn blocks(&self) -> u32 {
		self.blocks
	}

	fn transactions(&self) -> u32 {
		self.transactions
	}

	fn set_block_number(&mut self, value: u32) {
		self.block_number = value;
	}

	fn transfer_extrinsic(
		&mut self,
		sender: &Self::AccountId,
		key: &Self::Secret,
		destination: &Self::AccountId,
		amount: &Self::Balance,
		version: u32,
		genesis_hash: &<Self::Block as BlockT>::Hash,
		prior_block_hash: &<Self::Block as BlockT>::Hash,
	) -> <Self::Block as BlockT>::Extrinsic {
		let phase = self.block_number() as Self::Phase;
		let extra = Self::build_extra(self.index.into(), phase);
		self.index += 1;

		sign::<Self>(
			CheckedExtrinsic {
				signed: Some((sender.clone(), extra)),
				function: Call::GenericAsset(GenericAssetCall::transfer(
					SPENDING_ASSET_ID,
					destination.clone(),
					(*amount).into(),
				)),
			},
			key,
			(
				(),
				version,
				genesis_hash.clone(),
				prior_block_hash.clone(),
				(),
				(),
				(),
				(),
			),
		)
	}

	fn inherent_extrinsics(&self) -> InherentData {
		let timestamp = (self.block_number as u64 + 1) * MinimumPeriod::get();

		let mut inherent = InherentData::new();
		inherent
			.put_data(sp_timestamp::INHERENT_IDENTIFIER, &timestamp)
			.expect("Failed putting timestamp inherent");
		inherent
			.put_data(sp_finality_tracker::INHERENT_IDENTIFIER, &self.block_number)
			.expect("Failed putting finalized number inherent");
		inherent
	}

	fn minimum_balance() -> Self::Balance {
		Zero::zero()
	}

	fn master_account_id() -> Self::AccountId {
		Keyring::Alice.to_account_id()
	}

	fn master_account_secret() -> Self::Secret {
		Keyring::Alice.pair()
	}

	/// Generates a random `AccountId` from `seed`.
	fn gen_random_account_id(seed: u32) -> Self::AccountId {
		let pair: sr25519::Pair = sr25519::Pair::from_seed(&gen_seed_bytes(seed));
		AccountPublic::from(pair.public()).into_account()
	}

	/// Generates a random `Secret` from `seed`.
	fn gen_random_account_secret(seed: u32) -> Self::Secret {
		let pair: sr25519::Pair = sr25519::Pair::from_seed(&gen_seed_bytes(seed));
		pair
	}
}

fn gen_seed_bytes(seed: u32) -> [u8; 32] {
	let mut rng: StdRng = SeedableRng::seed_from_u64(seed as u64);

	let mut seed_bytes = [0u8; 32];
	for i in 0..32 {
		seed_bytes[i] = rng.gen::<u8>();
	}
	seed_bytes
}

/// Creates an `UncheckedExtrinsic` containing the appropriate signature for
/// a `CheckedExtrinsics`.
fn sign<RA: RuntimeAdapter>(
	xt: CheckedExtrinsic,
	key: &sr25519::Pair,
	additional_signed: <SignedExtra as SignedExtension>::AdditionalSigned,
) -> <RA::Block as BlockT>::Extrinsic {
	let s = match xt.signed {
		Some((signed, extra)) => {
			let payload = (xt.function, extra.clone(), additional_signed);
			let signature = payload
				.using_encoded(|b| {
					if b.len() > 256 {
						key.sign(&sp_io::hashing::blake2_256(b))
					} else {
						key.sign(b)
					}
				})
				.into();
			UncheckedExtrinsic {
				signature: Some((signed, signature, extra)),
				function: payload.0,
			}
		}
		None => UncheckedExtrinsic {
			signature: None,
			function: xt.function,
		},
	};

	let e = Encode::encode(&s);
	Decode::decode(&mut &e[..]).expect("Failed to decode signed unchecked extrinsic")
}
