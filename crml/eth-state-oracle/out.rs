#![feature(prelude_import)]
#[prelude_import]
use std::prelude::rust_2018::*;
#[macro_use]
extern crate std;
use cennznet_primitives::types::{Balance, FeePreferences};
use crml_support::{
    ContractExecutionInfo, ContractExecutor, EthAbiCodec, EthCallOracle, EthCallOracleSubscriber,
    EthereumStateOracle, MultiCurrency, H160,
};
use frame_support::{
    decl_error, decl_event, decl_module, decl_storage,
    dispatch::{DispatchErrorWithPostInfo, PostDispatchInfo},
    log,
    pallet_prelude::*,
    traits::{ExistenceRequirement, UnixTime},
    weights::{constants::RocksDbWeight as DbWeight, Pays, Weight},
};
use frame_system::ensure_signed;
use pallet_evm::AddressMapping;
use sp_runtime::traits::{SaturatedConversion, Zero};
use sp_std::prelude::*;
mod types {
    use cennznet_primitives::types::{Balance, FeePreferences};
    use codec::{Decode, Encode};
    pub use crml_support::{H160 as EthAddress, H256, U256};
    use scale_info::TypeInfo;
    #[doc = " Identifies remote call challenges"]
    pub type ChallengeId = u64;
    #[doc = " Identifies remote call requests"]
    pub type RequestId = U256;
    #[doc = " Details of a remote 'eth_call' request"]
    pub struct CallRequest {
        #[doc = " Destination address for the remote call"]
        pub destination: EthAddress,
        #[doc = " CENNZnet evm address of the caller"]
        pub caller: EthAddress,
        #[doc = " The gas limit for the callback execution"]
        pub callback_gas_limit: u64,
        #[doc = " Function selector of the callback"]
        pub callback_signature: [u8; 4],
        #[doc = " Fee preferences for callback gas payment"]
        pub fee_preferences: Option<FeePreferences>,
        #[doc = " A bounty for fulfiling the request successfully"]
        pub bounty: Balance,
        #[doc = " unix timestamp in seconds the request was placed"]
        pub timestamp: u64,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::fmt::Debug for CallRequest {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                CallRequest {
                    destination: ref __self_0_0,
                    caller: ref __self_0_1,
                    callback_gas_limit: ref __self_0_2,
                    callback_signature: ref __self_0_3,
                    fee_preferences: ref __self_0_4,
                    bounty: ref __self_0_5,
                    timestamp: ref __self_0_6,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "CallRequest");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "destination",
                        &&(*__self_0_0),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "caller",
                        &&(*__self_0_1),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "callback_gas_limit",
                        &&(*__self_0_2),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "callback_signature",
                        &&(*__self_0_3),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "fee_preferences",
                        &&(*__self_0_4),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "bounty",
                        &&(*__self_0_5),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "timestamp",
                        &&(*__self_0_6),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::clone::Clone for CallRequest {
        #[inline]
        fn clone(&self) -> CallRequest {
            match *self {
                CallRequest {
                    destination: ref __self_0_0,
                    caller: ref __self_0_1,
                    callback_gas_limit: ref __self_0_2,
                    callback_signature: ref __self_0_3,
                    fee_preferences: ref __self_0_4,
                    bounty: ref __self_0_5,
                    timestamp: ref __self_0_6,
                } => CallRequest {
                    destination: ::core::clone::Clone::clone(&(*__self_0_0)),
                    caller: ::core::clone::Clone::clone(&(*__self_0_1)),
                    callback_gas_limit: ::core::clone::Clone::clone(&(*__self_0_2)),
                    callback_signature: ::core::clone::Clone::clone(&(*__self_0_3)),
                    fee_preferences: ::core::clone::Clone::clone(&(*__self_0_4)),
                    bounty: ::core::clone::Clone::clone(&(*__self_0_5)),
                    timestamp: ::core::clone::Clone::clone(&(*__self_0_6)),
                },
            }
        }
    }
    impl ::core::marker::StructuralPartialEq for CallRequest {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl ::core::cmp::PartialEq for CallRequest {
        #[inline]
        fn eq(&self, other: &CallRequest) -> bool {
            match *other {
                CallRequest {
                    destination: ref __self_1_0,
                    caller: ref __self_1_1,
                    callback_gas_limit: ref __self_1_2,
                    callback_signature: ref __self_1_3,
                    fee_preferences: ref __self_1_4,
                    bounty: ref __self_1_5,
                    timestamp: ref __self_1_6,
                } => match *self {
                    CallRequest {
                        destination: ref __self_0_0,
                        caller: ref __self_0_1,
                        callback_gas_limit: ref __self_0_2,
                        callback_signature: ref __self_0_3,
                        fee_preferences: ref __self_0_4,
                        bounty: ref __self_0_5,
                        timestamp: ref __self_0_6,
                    } => {
                        (*__self_0_0) == (*__self_1_0)
                            && (*__self_0_1) == (*__self_1_1)
                            && (*__self_0_2) == (*__self_1_2)
                            && (*__self_0_3) == (*__self_1_3)
                            && (*__self_0_4) == (*__self_1_4)
                            && (*__self_0_5) == (*__self_1_5)
                            && (*__self_0_6) == (*__self_1_6)
                    }
                },
            }
        }
        #[inline]
        fn ne(&self, other: &CallRequest) -> bool {
            match *other {
                CallRequest {
                    destination: ref __self_1_0,
                    caller: ref __self_1_1,
                    callback_gas_limit: ref __self_1_2,
                    callback_signature: ref __self_1_3,
                    fee_preferences: ref __self_1_4,
                    bounty: ref __self_1_5,
                    timestamp: ref __self_1_6,
                } => match *self {
                    CallRequest {
                        destination: ref __self_0_0,
                        caller: ref __self_0_1,
                        callback_gas_limit: ref __self_0_2,
                        callback_signature: ref __self_0_3,
                        fee_preferences: ref __self_0_4,
                        bounty: ref __self_0_5,
                        timestamp: ref __self_0_6,
                    } => {
                        (*__self_0_0) != (*__self_1_0)
                            || (*__self_0_1) != (*__self_1_1)
                            || (*__self_0_2) != (*__self_1_2)
                            || (*__self_0_3) != (*__self_1_3)
                            || (*__self_0_4) != (*__self_1_4)
                            || (*__self_0_5) != (*__self_1_5)
                            || (*__self_0_6) != (*__self_1_6)
                    }
                },
            }
        }
    }
    const _: () = {
        impl ::codec::Decode for CallRequest {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(CallRequest {
                    destination: {
                        let __codec_res_edqy =
                            <EthAddress as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::destination`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    caller: {
                        let __codec_res_edqy =
                            <EthAddress as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::caller`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    callback_gas_limit: {
                        let __codec_res_edqy = <u64 as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::callback_gas_limit`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    callback_signature: {
                        let __codec_res_edqy =
                            <[u8; 4] as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::callback_signature`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    fee_preferences: {
                        let __codec_res_edqy =
                            <Option<FeePreferences> as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::fee_preferences`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    bounty: {
                        let __codec_res_edqy =
                            <Balance as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::bounty`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    timestamp: {
                        let __codec_res_edqy = <u64 as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallRequest::timestamp`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                })
            }
        }
    };
    const _: () = {
        impl ::codec::Encode for CallRequest {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&self.destination, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.caller, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.callback_gas_limit, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.callback_signature, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.fee_preferences, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.bounty, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.timestamp, __codec_dest_edqy);
            }
        }
        impl ::codec::EncodeLike for CallRequest {}
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl ::scale_info::TypeInfo for CallRequest {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "CallRequest",
                        "crml_eth_state_oracle::types",
                    ))
                    .type_params(::alloc::vec::Vec::new())
                    .docs(&["Details of a remote \'eth_call\' request"])
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| {
                                f.ty::<EthAddress>()
                                    .name("destination")
                                    .type_name("EthAddress")
                                    .docs(&["Destination address for the remote call"])
                            })
                            .field(|f| {
                                f.ty::<EthAddress>()
                                    .name("caller")
                                    .type_name("EthAddress")
                                    .docs(&["CENNZnet evm address of the caller"])
                            })
                            .field(|f| {
                                f.ty::<u64>()
                                    .name("callback_gas_limit")
                                    .type_name("u64")
                                    .docs(&["The gas limit for the callback execution"])
                            })
                            .field(|f| {
                                f.ty::<[u8; 4]>()
                                    .name("callback_signature")
                                    .type_name("[u8; 4]")
                                    .docs(&["Function selector of the callback"])
                            })
                            .field(|f| {
                                f.ty::<Option<FeePreferences>>()
                                    .name("fee_preferences")
                                    .type_name("Option<FeePreferences>")
                                    .docs(&["Fee preferences for callback gas payment"])
                            })
                            .field(|f| {
                                f.ty::<Balance>()
                                    .name("bounty")
                                    .type_name("Balance")
                                    .docs(&["A bounty for fulfiling the request successfully"])
                            })
                            .field(|f| {
                                f.ty::<u64>()
                                    .name("timestamp")
                                    .type_name("u64")
                                    .docs(&["unix timestamp in seconds the request was placed"])
                            }),
                    )
            }
        };
    };
    #[doc = " Reported response of an executed remote call"]
    pub struct CallResponse<AccountId> {
        #[doc = " Digest (blake256) of the return data"]
        pub return_data_digest: [u8; 32],
        #[doc = " The ethereum block number where the result was recorded"]
        pub eth_block_number: u64,
        #[doc = " Address of the relayer that reported this"]
        pub reporter: AccountId,
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl<AccountId: ::core::fmt::Debug> ::core::fmt::Debug for CallResponse<AccountId> {
        fn fmt(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
            match *self {
                CallResponse {
                    return_data_digest: ref __self_0_0,
                    eth_block_number: ref __self_0_1,
                    reporter: ref __self_0_2,
                } => {
                    let debug_trait_builder =
                        &mut ::core::fmt::Formatter::debug_struct(f, "CallResponse");
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "return_data_digest",
                        &&(*__self_0_0),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "eth_block_number",
                        &&(*__self_0_1),
                    );
                    let _ = ::core::fmt::DebugStruct::field(
                        debug_trait_builder,
                        "reporter",
                        &&(*__self_0_2),
                    );
                    ::core::fmt::DebugStruct::finish(debug_trait_builder)
                }
            }
        }
    }
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl<AccountId: ::core::clone::Clone> ::core::clone::Clone for CallResponse<AccountId> {
        #[inline]
        fn clone(&self) -> CallResponse<AccountId> {
            match *self {
                CallResponse {
                    return_data_digest: ref __self_0_0,
                    eth_block_number: ref __self_0_1,
                    reporter: ref __self_0_2,
                } => CallResponse {
                    return_data_digest: ::core::clone::Clone::clone(&(*__self_0_0)),
                    eth_block_number: ::core::clone::Clone::clone(&(*__self_0_1)),
                    reporter: ::core::clone::Clone::clone(&(*__self_0_2)),
                },
            }
        }
    }
    impl<AccountId> ::core::marker::StructuralPartialEq for CallResponse<AccountId> {}
    #[automatically_derived]
    #[allow(unused_qualifications)]
    impl<AccountId: ::core::cmp::PartialEq> ::core::cmp::PartialEq for CallResponse<AccountId> {
        #[inline]
        fn eq(&self, other: &CallResponse<AccountId>) -> bool {
            match *other {
                CallResponse {
                    return_data_digest: ref __self_1_0,
                    eth_block_number: ref __self_1_1,
                    reporter: ref __self_1_2,
                } => match *self {
                    CallResponse {
                        return_data_digest: ref __self_0_0,
                        eth_block_number: ref __self_0_1,
                        reporter: ref __self_0_2,
                    } => {
                        (*__self_0_0) == (*__self_1_0)
                            && (*__self_0_1) == (*__self_1_1)
                            && (*__self_0_2) == (*__self_1_2)
                    }
                },
            }
        }
        #[inline]
        fn ne(&self, other: &CallResponse<AccountId>) -> bool {
            match *other {
                CallResponse {
                    return_data_digest: ref __self_1_0,
                    eth_block_number: ref __self_1_1,
                    reporter: ref __self_1_2,
                } => match *self {
                    CallResponse {
                        return_data_digest: ref __self_0_0,
                        eth_block_number: ref __self_0_1,
                        reporter: ref __self_0_2,
                    } => {
                        (*__self_0_0) != (*__self_1_0)
                            || (*__self_0_1) != (*__self_1_1)
                            || (*__self_0_2) != (*__self_1_2)
                    }
                },
            }
        }
    }
    const _: () = {
        impl<AccountId> ::codec::Decode for CallResponse<AccountId>
        where
            AccountId: ::codec::Decode,
            AccountId: ::codec::Decode,
        {
            fn decode<__CodecInputEdqy: ::codec::Input>(
                __codec_input_edqy: &mut __CodecInputEdqy,
            ) -> ::core::result::Result<Self, ::codec::Error> {
                ::core::result::Result::Ok(CallResponse::<AccountId> {
                    return_data_digest: {
                        let __codec_res_edqy =
                            <[u8; 32] as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallResponse::return_data_digest`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    eth_block_number: {
                        let __codec_res_edqy = <u64 as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallResponse::eth_block_number`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                    reporter: {
                        let __codec_res_edqy =
                            <AccountId as ::codec::Decode>::decode(__codec_input_edqy);
                        match __codec_res_edqy {
                            ::core::result::Result::Err(e) => {
                                return ::core::result::Result::Err(
                                    e.chain("Could not decode `CallResponse::reporter`"),
                                )
                            }
                            ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                        }
                    },
                })
            }
        }
    };
    const _: () = {
        impl<AccountId> ::codec::Encode for CallResponse<AccountId>
        where
            AccountId: ::codec::Encode,
            AccountId: ::codec::Encode,
        {
            fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
                &self,
                __codec_dest_edqy: &mut __CodecOutputEdqy,
            ) {
                ::codec::Encode::encode_to(&self.return_data_digest, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.eth_block_number, __codec_dest_edqy);
                ::codec::Encode::encode_to(&self.reporter, __codec_dest_edqy);
            }
        }
        impl<AccountId> ::codec::EncodeLike for CallResponse<AccountId>
        where
            AccountId: ::codec::Encode,
            AccountId: ::codec::Encode,
        {
        }
    };
    #[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
    const _: () = {
        impl<AccountId> ::scale_info::TypeInfo for CallResponse<AccountId>
        where
            AccountId: ::scale_info::TypeInfo + 'static,
            AccountId: ::scale_info::TypeInfo + 'static,
        {
            type Identity = Self;
            fn type_info() -> ::scale_info::Type {
                ::scale_info::Type::builder()
                    .path(::scale_info::Path::new(
                        "CallResponse",
                        "crml_eth_state_oracle::types",
                    ))
                    .type_params(<[_]>::into_vec(box [::scale_info::TypeParameter::new(
                        "AccountId",
                        ::core::option::Option::Some(::scale_info::meta_type::<AccountId>()),
                    )]))
                    .docs(&["Reported response of an executed remote call"])
                    .composite(
                        ::scale_info::build::Fields::named()
                            .field(|f| {
                                f.ty::<[u8; 32]>()
                                    .name("return_data_digest")
                                    .type_name("[u8; 32]")
                                    .docs(&["Digest (blake256) of the return data"])
                            })
                            .field(|f| {
                                f.ty::<u64>()
                                    .name("eth_block_number")
                                    .type_name("u64")
                                    .docs(&[
                                        "The ethereum block number where the result was recorded",
                                    ])
                            })
                            .field(|f| {
                                f.ty::<AccountId>()
                                    .name("reporter")
                                    .type_name("AccountId")
                                    .docs(&["Address of the relayer that reported this"])
                            }),
                    )
            }
        };
    };
}
use types::*;
pub(crate) const LOG_TARGET: &str = "state-oracle";
pub trait Config: frame_system::Config {
    #[doc = " Map evm address into ss58 address"]
    type AddressMapping: AddressMapping<Self::AccountId>;
    #[doc = " Challenge period in blocks for state oracle responses"]
    type ChallengePeriod: Get<Self::BlockNumber>;
    #[doc = " Handles invoking request callbacks"]
    type ContractExecutor: ContractExecutor<Address = EthAddress>;
    #[doc = " Configured address for the state oracle precompile"]
    type StateOraclePrecompileAddress: Get<H160>;
    #[doc = " Handles verifying challenged responses"]
    type EthCallOracle: EthCallOracle<Address = EthAddress, CallId = u64>;
    #[doc = " The overarching event type."]
    type Event: From<Event> + IsType<<Self as frame_system::Config>::Event>;
    #[doc = " Returns current block time"]
    type UnixTime: UnixTime;
    #[doc = " Multi-currency system"]
    type MultiCurrency: MultiCurrency<AccountId = Self::AccountId, Balance = Balance>;
    #[doc = " Returns the network min gas price"]
    type MinGasPrice: Get<u64>;
}
use self::sp_api_hidden_includes_decl_storage::hidden_include::{
    StorageValue as _, StorageMap as _, StorageDoubleMap as _, StorageNMap as _,
    StoragePrefixedMap as _, IterableStorageMap as _, IterableStorageNMap as _,
    IterableStorageDoubleMap as _,
};
#[doc(hidden)]
mod sp_api_hidden_includes_decl_storage {
    pub extern crate frame_support as hidden_include;
}
trait Store {
    type ChallengeSubscriptions;
    type NextRequestId;
    type Requests;
    type RequestInputData;
    type ResponseReturnData;
    type Responses;
    type ResponsesChallenged;
    type ResponsesValidAtBlock;
    type ResponsesForCallback;
}
impl<T: Config + 'static> Store for Module<T> {
    type ChallengeSubscriptions = ChallengeSubscriptions;
    type NextRequestId = NextRequestId;
    type Requests = Requests;
    type RequestInputData = RequestInputData;
    type ResponseReturnData = ResponseReturnData;
    type Responses = Responses<T>;
    type ResponsesChallenged = ResponsesChallenged<T>;
    type ResponsesValidAtBlock = ResponsesValidAtBlock<T>;
    type ResponsesForCallback = ResponsesForCallback;
}
impl<T: Config + 'static> Module<T> {
    #[doc = " Unique identifier for remote call requests"]
    pub fn next_request_id() -> RequestId {
        < NextRequestId < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StorageValue < RequestId > > :: get ()
    }
    #[doc = " Requests for remote \'eth_call\'s keyed by request Id"]
    pub fn requests<
        K: self::sp_api_hidden_includes_decl_storage::hidden_include::codec::EncodeLike<RequestId>,
    >(
        key: K,
    ) -> Option<CallRequest> {
        < Requests < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StorageMap < RequestId , CallRequest > > :: get (key)
    }
}
#[doc(hidden)]
pub struct __GetByteStructChallengeSubscriptions<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_ChallengeSubscriptions:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructChallengeSubscriptions<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_ChallengeSubscriptions
            .get_or_init(|| {
                let def_val: Option<RequestId> = Default::default();
                <Option<RequestId> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructNextRequestId<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_NextRequestId:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructNextRequestId<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_NextRequestId
            .get_or_init(|| {
                let def_val: RequestId = Default::default();
                <RequestId as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructRequests<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_Requests:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructRequests<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_Requests
            .get_or_init(|| {
                let def_val: Option<CallRequest> = Default::default();
                <Option<CallRequest> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructRequestInputData<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_RequestInputData:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructRequestInputData<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_RequestInputData
            .get_or_init(|| {
                let def_val: Vec<u8> = Default::default();
                <Vec<u8> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructResponseReturnData<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_ResponseReturnData:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructResponseReturnData<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_ResponseReturnData
            .get_or_init(|| {
                let def_val: Vec<u8> = Default::default();
                <Vec<u8> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructResponses<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_Responses:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructResponses<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_Responses
            .get_or_init(|| {
                let def_val: Option<CallResponse<T::AccountId>> = Default::default();
                <Option<CallResponse<T::AccountId>> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructResponsesChallenged<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_ResponsesChallenged:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructResponsesChallenged<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_ResponsesChallenged
            .get_or_init(|| {
                let def_val: Option<T::AccountId> = Default::default();
                <Option<T::AccountId> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructResponsesValidAtBlock<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_ResponsesValidAtBlock:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructResponsesValidAtBlock<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_ResponsesValidAtBlock
            .get_or_init(|| {
                let def_val: Vec<RequestId> = Default::default();
                <Vec<RequestId> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
#[doc(hidden)]
pub struct __GetByteStructResponsesForCallback<T>(
    pub self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T)>,
);
#[cfg(feature = "std")]
#[allow(non_upper_case_globals)]
static __CACHE_GET_BYTE_STRUCT_ResponsesForCallback:
    self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell<
        self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8>,
    > = self::sp_api_hidden_includes_decl_storage::hidden_include::once_cell::sync::OnceCell::new();
#[cfg(feature = "std")]
impl<T: Config> __GetByteStructResponsesForCallback<T> {
    fn default_byte(
        &self,
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<u8> {
        use self::sp_api_hidden_includes_decl_storage::hidden_include::codec::Encode;
        __CACHE_GET_BYTE_STRUCT_ResponsesForCallback
            .get_or_init(|| {
                let def_val: Vec<RequestId> = Default::default();
                <Vec<RequestId> as Encode>::encode(&def_val)
            })
            .clone()
    }
}
impl<T: Config + 'static> Module<T> {
    #[doc(hidden)]
    pub fn storage_metadata(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::metadata::PalletStorageMetadata
    {
        self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: PalletStorageMetadata { prefix : "EthStateOracle" , entries : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "ChallengeSubscriptions" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Optional , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < ChallengeId > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > () , } , default : __GetByteStructChallengeSubscriptions :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Map from challenge subscription Id to its request"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "NextRequestId" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Default , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Plain (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > ()) , default : __GetByteStructNextRequestId :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Unique identifier for remote call requests"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "Requests" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Optional , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < CallRequest > () , } , default : __GetByteStructRequests :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Requests for remote \'eth_call\'s keyed by request Id"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "RequestInputData" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Default , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < Vec < u8 > > () , } , default : __GetByteStructRequestInputData :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Input data for remote calls keyed by request Id"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "ResponseReturnData" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Default , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < Vec < u8 > > () , } , default : __GetByteStructResponseReturnData :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Reported return data keyed by request Id"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "Responses" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Optional , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < CallResponse < T :: AccountId > > () , } , default : __GetByteStructResponses :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Reported response details keyed by request Id" , " These are not necessarily valid until passed the challenge period"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "ResponsesChallenged" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Optional , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < RequestId > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < T :: AccountId > () , } , default : __GetByteStructResponsesChallenged :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Responses that are being actively challenged (value is the challenger)"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "ResponsesValidAtBlock" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Default , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Map { hashers : < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageHasher :: Twox64Concat]) , key : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < T :: BlockNumber > () , value : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < Vec < RequestId > > () , } , default : __GetByteStructResponsesValidAtBlock :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Map from block numbers to a list of responses that will be valid at the block (i.e. past the challenged period)"]) , } , self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryMetadata { name : "ResponsesForCallback" , modifier : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryModifier :: Default , ty : self :: sp_api_hidden_includes_decl_storage :: hidden_include :: metadata :: StorageEntryType :: Plain (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: scale_info :: meta_type :: < Vec < RequestId > > ()) , default : __GetByteStructResponsesForCallback :: < T > (self :: sp_api_hidden_includes_decl_storage :: hidden_include :: sp_std :: marker :: PhantomData) . default_byte () , docs : < [_] > :: into_vec (box [" Queue of validated responses ready to issue callbacks"]) , }]) , }
    }
}
#[doc = r" Hidden instance generated to be internally used when module is used without"]
#[doc = r" instance."]
#[doc(hidden)]
pub struct __InherentHiddenInstance;
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::clone::Clone for __InherentHiddenInstance {
    #[inline]
    fn clone(&self) -> __InherentHiddenInstance {
        match *self {
            __InherentHiddenInstance => __InherentHiddenInstance,
        }
    }
}
impl ::core::marker::StructuralEq for __InherentHiddenInstance {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::cmp::Eq for __InherentHiddenInstance {
    #[inline]
    #[doc(hidden)]
    #[no_coverage]
    fn assert_receiver_is_total_eq(&self) -> () {
        {}
    }
}
impl ::core::marker::StructuralPartialEq for __InherentHiddenInstance {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::cmp::PartialEq for __InherentHiddenInstance {
    #[inline]
    fn eq(&self, other: &__InherentHiddenInstance) -> bool {
        match *other {
            __InherentHiddenInstance => match *self {
                __InherentHiddenInstance => true,
            },
        }
    }
}
const _: () = {
    impl ::codec::Encode for __InherentHiddenInstance {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
        }
    }
    impl ::codec::EncodeLike for __InherentHiddenInstance {}
};
const _: () = {
    impl ::codec::Decode for __InherentHiddenInstance {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            ::core::result::Result::Ok(__InherentHiddenInstance)
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl ::scale_info::TypeInfo for __InherentHiddenInstance {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new(
                    "__InherentHiddenInstance",
                    "crml_eth_state_oracle",
                ))
                .type_params(::alloc::vec::Vec::new())
                .docs(&[
                    "Hidden instance generated to be internally used when module is used without",
                    "instance.",
                ])
                .composite(::scale_info::build::Fields::unit())
        }
    };
};
impl core::fmt::Debug for __InherentHiddenInstance {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_tuple("__InherentHiddenInstance").finish()
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::Instance
    for __InherentHiddenInstance
{
    const PREFIX: &'static str = "EthStateOracle";
    const INDEX: u8 = 0u8;
}
#[doc = " Map from challenge subscription Id to its request"]
struct ChallengeSubscriptions(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<()>,
);
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<
        RequestId,
    > for ChallengeSubscriptions
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ChallengeSubscriptions"
    }
}
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        ChallengeId,
        RequestId,
    > for ChallengeSubscriptions
{
    type Query = Option<RequestId>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ChallengeSubscriptions"
    }
    fn from_optional_value_to_query(v: Option<RequestId>) -> Self::Query {
        v.or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<RequestId> {
        v
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for ChallengeSubscriptions
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < ChallengeSubscriptions < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < RequestId > > :: module_prefix () . to_vec () , storage_name : < ChallengeSubscriptions < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < RequestId > > :: storage_prefix () . to_vec () , prefix : < ChallengeSubscriptions < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < RequestId > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Unique identifier for remote call requests"]
struct NextRequestId(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<()>,
);
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageValue<
        RequestId,
    > for NextRequestId
{
    type Query = RequestId;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"NextRequestId"
    }
    fn from_optional_value_to_query(v: Option<RequestId>) -> Self::Query {
        v.unwrap_or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<RequestId> {
        Some(v)
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for NextRequestId
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < NextRequestId < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: generator :: StorageValue < RequestId > > :: module_prefix () . to_vec () , storage_name : < NextRequestId < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: generator :: StorageValue < RequestId > > :: storage_prefix () . to_vec () , prefix : < NextRequestId < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: generator :: StorageValue < RequestId > > :: storage_value_final_key () . to_vec () , max_values : Some (1) , max_size : None , }])
    }
}
#[doc = " Requests for remote \'eth_call\'s keyed by request Id"]
struct Requests(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<()>,
);
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<
        CallRequest,
    > for Requests
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"Requests"
    }
}
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        RequestId,
        CallRequest,
    > for Requests
{
    type Query = Option<CallRequest>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"Requests"
    }
    fn from_optional_value_to_query(v: Option<CallRequest>) -> Self::Query {
        v.or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<CallRequest> {
        v
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for Requests
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < Requests < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < CallRequest > > :: module_prefix () . to_vec () , storage_name : < Requests < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < CallRequest > > :: storage_prefix () . to_vec () , prefix : < Requests < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < CallRequest > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Input data for remote calls keyed by request Id"]
struct RequestInputData(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<()>,
);
impl self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<Vec<u8>>
    for RequestInputData
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"RequestInputData"
    }
}
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        RequestId,
        Vec<u8>,
    > for RequestInputData
{
    type Query = Vec<u8>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"RequestInputData"
    }
    fn from_optional_value_to_query(v: Option<Vec<u8>>) -> Self::Query {
        v.unwrap_or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<Vec<u8>> {
        Some(v)
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for RequestInputData
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < RequestInputData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < u8 > > > :: module_prefix () . to_vec () , storage_name : < RequestInputData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < u8 > > > :: storage_prefix () . to_vec () , prefix : < RequestInputData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < u8 > > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Reported return data keyed by request Id"]
struct ResponseReturnData(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<()>,
);
impl self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<Vec<u8>>
    for ResponseReturnData
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponseReturnData"
    }
}
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        RequestId,
        Vec<u8>,
    > for ResponseReturnData
{
    type Query = Vec<u8>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponseReturnData"
    }
    fn from_optional_value_to_query(v: Option<Vec<u8>>) -> Self::Query {
        v.unwrap_or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<Vec<u8>> {
        Some(v)
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for ResponseReturnData
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < ResponseReturnData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < u8 > > > :: module_prefix () . to_vec () , storage_name : < ResponseReturnData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < u8 > > > :: storage_prefix () . to_vec () , prefix : < ResponseReturnData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < u8 > > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Reported response details keyed by request Id"]
#[doc = " These are not necessarily valid until passed the challenge period"]
struct Responses<T: Config>(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T,)>,
);
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<
        CallResponse<T::AccountId>,
    > for Responses<T>
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"Responses"
    }
}
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        RequestId,
        CallResponse<T::AccountId>,
    > for Responses<T>
{
    type Query = Option<CallResponse<T::AccountId>>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"Responses"
    }
    fn from_optional_value_to_query(v: Option<CallResponse<T::AccountId>>) -> Self::Query {
        v.or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<CallResponse<T::AccountId>> {
        v
    }
}
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for Responses<T>
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < Responses < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < CallResponse < T :: AccountId > > > :: module_prefix () . to_vec () , storage_name : < Responses < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < CallResponse < T :: AccountId > > > :: storage_prefix () . to_vec () , prefix : < Responses < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < CallResponse < T :: AccountId > > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Responses that are being actively challenged (value is the challenger)"]
struct ResponsesChallenged<T: Config>(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T,)>,
);
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<
        T::AccountId,
    > for ResponsesChallenged<T>
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponsesChallenged"
    }
}
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        RequestId,
        T::AccountId,
    > for ResponsesChallenged<T>
{
    type Query = Option<T::AccountId>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponsesChallenged"
    }
    fn from_optional_value_to_query(v: Option<T::AccountId>) -> Self::Query {
        v.or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<T::AccountId> {
        v
    }
}
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for ResponsesChallenged<T>
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < ResponsesChallenged < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < T :: AccountId > > :: module_prefix () . to_vec () , storage_name : < ResponsesChallenged < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < T :: AccountId > > :: storage_prefix () . to_vec () , prefix : < ResponsesChallenged < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < T :: AccountId > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Map from block numbers to a list of responses that will be valid at the block (i.e. past the challenged period)"]
struct ResponsesValidAtBlock<T: Config>(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<(T,)>,
);
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::StoragePrefixedMap<
        Vec<RequestId>,
    > for ResponsesValidAtBlock<T>
{
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponsesValidAtBlock"
    }
}
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageMap<
        T::BlockNumber,
        Vec<RequestId>,
    > for ResponsesValidAtBlock<T>
{
    type Query = Vec<RequestId>;
    type Hasher = self::sp_api_hidden_includes_decl_storage::hidden_include::Twox64Concat;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponsesValidAtBlock"
    }
    fn from_optional_value_to_query(v: Option<Vec<RequestId>>) -> Self::Query {
        v.unwrap_or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<Vec<RequestId>> {
        Some(v)
    }
}
impl<T: Config>
    self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for ResponsesValidAtBlock<T>
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < ResponsesValidAtBlock < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < RequestId > > > :: module_prefix () . to_vec () , storage_name : < ResponsesValidAtBlock < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < RequestId > > > :: storage_prefix () . to_vec () , prefix : < ResponsesValidAtBlock < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: StoragePrefixedMap < Vec < RequestId > > > :: final_prefix () . to_vec () , max_values : None , max_size : None , }])
    }
}
#[doc = " Queue of validated responses ready to issue callbacks"]
struct ResponsesForCallback(
    self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::marker::PhantomData<()>,
);
impl
    self::sp_api_hidden_includes_decl_storage::hidden_include::storage::generator::StorageValue<
        Vec<RequestId>,
    > for ResponsesForCallback
{
    type Query = Vec<RequestId>;
    fn module_prefix() -> &'static [u8] {
        < __InherentHiddenInstance as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: Instance > :: PREFIX . as_bytes ()
    }
    fn storage_prefix() -> &'static [u8] {
        b"ResponsesForCallback"
    }
    fn from_optional_value_to_query(v: Option<Vec<RequestId>>) -> Self::Query {
        v.unwrap_or_else(|| Default::default())
    }
    fn from_query_to_optional_value(v: Self::Query) -> Option<Vec<RequestId>> {
        Some(v)
    }
}
impl self::sp_api_hidden_includes_decl_storage::hidden_include::traits::PartialStorageInfoTrait
    for ResponsesForCallback
{
    fn partial_storage_info(
    ) -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        < [_] > :: into_vec (box [self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: StorageInfo { pallet_name : < ResponsesForCallback < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: generator :: StorageValue < Vec < RequestId > > > :: module_prefix () . to_vec () , storage_name : < ResponsesForCallback < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: generator :: StorageValue < Vec < RequestId > > > :: storage_prefix () . to_vec () , prefix : < ResponsesForCallback < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: storage :: generator :: StorageValue < Vec < RequestId > > > :: storage_value_final_key () . to_vec () , max_values : Some (1) , max_size : None , }])
    }
}
impl<T: Config + 'static>
    self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfoTrait
    for Module<T>
{
    fn storage_info() -> self::sp_api_hidden_includes_decl_storage::hidden_include::sp_std::vec::Vec<
        self::sp_api_hidden_includes_decl_storage::hidden_include::traits::StorageInfo,
    > {
        let mut res = ::alloc::vec::Vec::new();
        let mut storage_info = < ChallengeSubscriptions < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < NextRequestId < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < Requests < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < RequestInputData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < ResponseReturnData < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < Responses < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < ResponsesChallenged < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < ResponsesValidAtBlock < T > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        let mut storage_info = < ResponsesForCallback < > as self :: sp_api_hidden_includes_decl_storage :: hidden_include :: traits :: PartialStorageInfoTrait > :: partial_storage_info () ;
        res.append(&mut storage_info);
        res
    }
}
#[scale_info(capture_docs = "always")]
#[doc = " Events for this module."]
#[doc = ""]
pub enum Event {
    #[doc = r" New state oracle request (Caller, Id)"]
    NewRequest(EthAddress, RequestId),
    #[doc = r" executing the request callback failed (Id, Reason)"]
    CallbackErr(RequestId, DispatchError),
    #[doc = r" executing the callback succeeded (Id, Weight)"]
    Callback(RequestId, Weight),
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::clone::Clone for Event {
    #[inline]
    fn clone(&self) -> Event {
        match (&*self,) {
            (&Event::NewRequest(ref __self_0, ref __self_1),) => Event::NewRequest(
                ::core::clone::Clone::clone(&(*__self_0)),
                ::core::clone::Clone::clone(&(*__self_1)),
            ),
            (&Event::CallbackErr(ref __self_0, ref __self_1),) => Event::CallbackErr(
                ::core::clone::Clone::clone(&(*__self_0)),
                ::core::clone::Clone::clone(&(*__self_1)),
            ),
            (&Event::Callback(ref __self_0, ref __self_1),) => Event::Callback(
                ::core::clone::Clone::clone(&(*__self_0)),
                ::core::clone::Clone::clone(&(*__self_1)),
            ),
        }
    }
}
impl ::core::marker::StructuralPartialEq for Event {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::cmp::PartialEq for Event {
    #[inline]
    fn eq(&self, other: &Event) -> bool {
        {
            let __self_vi = ::core::intrinsics::discriminant_value(&*self);
            let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
            if true && __self_vi == __arg_1_vi {
                match (&*self, &*other) {
                    (
                        &Event::NewRequest(ref __self_0, ref __self_1),
                        &Event::NewRequest(ref __arg_1_0, ref __arg_1_1),
                    ) => (*__self_0) == (*__arg_1_0) && (*__self_1) == (*__arg_1_1),
                    (
                        &Event::CallbackErr(ref __self_0, ref __self_1),
                        &Event::CallbackErr(ref __arg_1_0, ref __arg_1_1),
                    ) => (*__self_0) == (*__arg_1_0) && (*__self_1) == (*__arg_1_1),
                    (
                        &Event::Callback(ref __self_0, ref __self_1),
                        &Event::Callback(ref __arg_1_0, ref __arg_1_1),
                    ) => (*__self_0) == (*__arg_1_0) && (*__self_1) == (*__arg_1_1),
                    _ => unsafe { ::core::intrinsics::unreachable() },
                }
            } else {
                false
            }
        }
    }
    #[inline]
    fn ne(&self, other: &Event) -> bool {
        {
            let __self_vi = ::core::intrinsics::discriminant_value(&*self);
            let __arg_1_vi = ::core::intrinsics::discriminant_value(&*other);
            if true && __self_vi == __arg_1_vi {
                match (&*self, &*other) {
                    (
                        &Event::NewRequest(ref __self_0, ref __self_1),
                        &Event::NewRequest(ref __arg_1_0, ref __arg_1_1),
                    ) => (*__self_0) != (*__arg_1_0) || (*__self_1) != (*__arg_1_1),
                    (
                        &Event::CallbackErr(ref __self_0, ref __self_1),
                        &Event::CallbackErr(ref __arg_1_0, ref __arg_1_1),
                    ) => (*__self_0) != (*__arg_1_0) || (*__self_1) != (*__arg_1_1),
                    (
                        &Event::Callback(ref __self_0, ref __self_1),
                        &Event::Callback(ref __arg_1_0, ref __arg_1_1),
                    ) => (*__self_0) != (*__arg_1_0) || (*__self_1) != (*__arg_1_1),
                    _ => unsafe { ::core::intrinsics::unreachable() },
                }
            } else {
                true
            }
        }
    }
}
impl ::core::marker::StructuralEq for Event {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl ::core::cmp::Eq for Event {
    #[inline]
    #[doc(hidden)]
    #[no_coverage]
    fn assert_receiver_is_total_eq(&self) -> () {
        {
            let _: ::core::cmp::AssertParamIsEq<EthAddress>;
            let _: ::core::cmp::AssertParamIsEq<RequestId>;
            let _: ::core::cmp::AssertParamIsEq<RequestId>;
            let _: ::core::cmp::AssertParamIsEq<DispatchError>;
            let _: ::core::cmp::AssertParamIsEq<RequestId>;
            let _: ::core::cmp::AssertParamIsEq<Weight>;
        }
    }
}
const _: () = {
    impl ::codec::Encode for Event {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                Event::NewRequest(ref aa, ref ba) => {
                    __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    ::codec::Encode::encode_to(ba, __codec_dest_edqy);
                }
                Event::CallbackErr(ref aa, ref ba) => {
                    __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    ::codec::Encode::encode_to(ba, __codec_dest_edqy);
                }
                Event::Callback(ref aa, ref ba) => {
                    __codec_dest_edqy.push_byte(2usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(aa, __codec_dest_edqy);
                    ::codec::Encode::encode_to(ba, __codec_dest_edqy);
                }
                _ => (),
            }
        }
    }
    impl ::codec::EncodeLike for Event {}
};
const _: () = {
    impl ::codec::Decode for Event {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy
                .read_byte()
                .map_err(|e| e.chain("Could not decode `Event`, failed to read variant byte"))?
            {
                __codec_x_edqy if __codec_x_edqy == 0usize as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(Event::NewRequest(
                        {
                            let __codec_res_edqy =
                                <EthAddress as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `Event::NewRequest.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                        {
                            let __codec_res_edqy =
                                <RequestId as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `Event::NewRequest.1`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                    ))
                }
                __codec_x_edqy if __codec_x_edqy == 1usize as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(Event::CallbackErr(
                        {
                            let __codec_res_edqy =
                                <RequestId as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `Event::CallbackErr.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                        {
                            let __codec_res_edqy =
                                <DispatchError as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `Event::CallbackErr.1`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                    ))
                }
                __codec_x_edqy if __codec_x_edqy == 2usize as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(Event::Callback(
                        {
                            let __codec_res_edqy =
                                <RequestId as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `Event::Callback.0`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                        {
                            let __codec_res_edqy =
                                <Weight as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(
                                        e.chain("Could not decode `Event::Callback.1`"),
                                    )
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                    ))
                }
                _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                    "Could not decode `Event`, variant doesn\'t exist",
                )),
            }
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl ::scale_info::TypeInfo for Event {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new("Event", "crml_eth_state_oracle"))
                .type_params(::alloc::vec::Vec::new())
                .docs_always(&["Events for this module.", ""])
                .variant(
                    ::scale_info::build::Variants::new()
                        .variant("NewRequest", |v| {
                            v.index(0usize as ::core::primitive::u8)
                                .fields(
                                    ::scale_info::build::Fields::unnamed()
                                        .field(|f| {
                                            f.ty::<EthAddress>()
                                                .type_name("EthAddress")
                                                .docs_always(&[])
                                        })
                                        .field(|f| {
                                            f.ty::<RequestId>()
                                                .type_name("RequestId")
                                                .docs_always(&[])
                                        }),
                                )
                                .docs_always(&["New state oracle request (Caller, Id)"])
                        })
                        .variant("CallbackErr", |v| {
                            v.index(1usize as ::core::primitive::u8)
                                .fields(
                                    ::scale_info::build::Fields::unnamed()
                                        .field(|f| {
                                            f.ty::<RequestId>()
                                                .type_name("RequestId")
                                                .docs_always(&[])
                                        })
                                        .field(|f| {
                                            f.ty::<DispatchError>()
                                                .type_name("DispatchError")
                                                .docs_always(&[])
                                        }),
                                )
                                .docs_always(&[
                                    "executing the request callback failed (Id, Reason)",
                                ])
                        })
                        .variant("Callback", |v| {
                            v.index(2usize as ::core::primitive::u8)
                                .fields(
                                    ::scale_info::build::Fields::unnamed()
                                        .field(|f| {
                                            f.ty::<RequestId>()
                                                .type_name("RequestId")
                                                .docs_always(&[])
                                        })
                                        .field(|f| {
                                            f.ty::<Weight>().type_name("Weight").docs_always(&[])
                                        }),
                                )
                                .docs_always(&["executing the callback succeeded (Id, Weight)"])
                        }),
                )
        }
    };
};
impl core::fmt::Debug for Event {
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        match self {
            Self::NewRequest(ref a0, ref a1) => fmt
                .debug_tuple("Event::NewRequest")
                .field(a0)
                .field(a1)
                .finish(),
            Self::CallbackErr(ref a0, ref a1) => fmt
                .debug_tuple("Event::CallbackErr")
                .field(a0)
                .field(a1)
                .finish(),
            Self::Callback(ref a0, ref a1) => fmt
                .debug_tuple("Event::Callback")
                .field(a0)
                .field(a1)
                .finish(),
            _ => Ok(()),
        }
    }
}
impl From<Event> for () {
    fn from(_: Event) -> () {
        ()
    }
}
#[scale_info(skip_type_params(T), capture_docs = "always")]
pub enum Error<T: Config> {
    #[doc(hidden)]
    #[codec(skip)]
    __Ignore(
        ::frame_support::sp_std::marker::PhantomData<(T,)>,
        ::frame_support::Never,
    ),
    #[doc = r" A response has already been submitted for this request"]
    ResponseExists,
    #[doc = r" The request does not exist (either fulfilled or never did)"]
    NoRequest,
    #[doc = r" No response exists"]
    NoResponse,
    #[doc = r" Paying for callback gas failed"]
    InsufficientFundsGas,
    #[doc = r" Paying the callback bounty to relayer failed"]
    InsufficientFundsBounty,
    #[doc = r" Challenge already in progress"]
    DuplicateChallenge,
}
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl<T: Config> ::scale_info::TypeInfo for Error<T>
    where
        ::frame_support::sp_std::marker::PhantomData<(T,)>: ::scale_info::TypeInfo + 'static,
        T: Config + 'static,
    {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            ::scale_info::Type::builder()
                .path(::scale_info::Path::new("Error", "crml_eth_state_oracle"))
                .type_params(<[_]>::into_vec(box [::scale_info::TypeParameter::new(
                    "T",
                    ::core::option::Option::None,
                )]))
                .docs_always(&[])
                .variant(
                    ::scale_info::build::Variants::new()
                        .variant("ResponseExists", |v| {
                            v.index(0usize as ::core::primitive::u8).docs_always(&[
                                "A response has already been submitted for this request",
                            ])
                        })
                        .variant("NoRequest", |v| {
                            v.index(1usize as ::core::primitive::u8).docs_always(&[
                                "The request does not exist (either fulfilled or never did)",
                            ])
                        })
                        .variant("NoResponse", |v| {
                            v.index(2usize as ::core::primitive::u8)
                                .docs_always(&["No response exists"])
                        })
                        .variant("InsufficientFundsGas", |v| {
                            v.index(3usize as ::core::primitive::u8)
                                .docs_always(&["Paying for callback gas failed"])
                        })
                        .variant("InsufficientFundsBounty", |v| {
                            v.index(4usize as ::core::primitive::u8)
                                .docs_always(&["Paying the callback bounty to relayer failed"])
                        })
                        .variant("DuplicateChallenge", |v| {
                            v.index(5usize as ::core::primitive::u8)
                                .docs_always(&["Challenge already in progress"])
                        }),
                )
        }
    };
};
impl<T: Config> ::frame_support::sp_std::fmt::Debug for Error<T> {
    fn fmt(
        &self,
        f: &mut ::frame_support::sp_std::fmt::Formatter<'_>,
    ) -> ::frame_support::sp_std::fmt::Result {
        f.write_str(self.as_str())
    }
}
impl<T: Config> Error<T> {
    fn as_u8(&self) -> u8 {
        match self {
            Error::__Ignore(_, _) => ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                &["internal error: entered unreachable code: "],
                &match (&"`__Ignore` can never be constructed",) {
                    (arg0,) => [::core::fmt::ArgumentV1::new(
                        arg0,
                        ::core::fmt::Display::fmt,
                    )],
                },
            )),
            Error::ResponseExists => 0,
            Error::NoRequest => 0 + 1,
            Error::NoResponse => 0 + 1 + 1,
            Error::InsufficientFundsGas => 0 + 1 + 1 + 1,
            Error::InsufficientFundsBounty => 0 + 1 + 1 + 1 + 1,
            Error::DuplicateChallenge => 0 + 1 + 1 + 1 + 1 + 1,
        }
    }
    fn as_str(&self) -> &'static str {
        match self {
            Self::__Ignore(_, _) => ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                &["internal error: entered unreachable code: "],
                &match (&"`__Ignore` can never be constructed",) {
                    (arg0,) => [::core::fmt::ArgumentV1::new(
                        arg0,
                        ::core::fmt::Display::fmt,
                    )],
                },
            )),
            Error::ResponseExists => "ResponseExists",
            Error::NoRequest => "NoRequest",
            Error::NoResponse => "NoResponse",
            Error::InsufficientFundsGas => "InsufficientFundsGas",
            Error::InsufficientFundsBounty => "InsufficientFundsBounty",
            Error::DuplicateChallenge => "DuplicateChallenge",
        }
    }
}
impl<T: Config> From<Error<T>> for &'static str {
    fn from(err: Error<T>) -> &'static str {
        err.as_str()
    }
}
impl<T: Config> From<Error<T>> for ::frame_support::sp_runtime::DispatchError {
    fn from(err: Error<T>) -> Self {
        let index = <T::PalletInfo as ::frame_support::traits::PalletInfo>::index::<Module<T>>()
            .expect("Every active module has an index in the runtime; qed")
            as u8;
        ::frame_support::sp_runtime::DispatchError::Module {
            index,
            error: err.as_u8(),
            message: Some(err.as_str()),
        }
    }
}
pub struct Module<T: Config>(::frame_support::sp_std::marker::PhantomData<(T,)>);
#[automatically_derived]
#[allow(unused_qualifications)]
impl<T: ::core::clone::Clone + Config> ::core::clone::Clone for Module<T> {
    #[inline]
    fn clone(&self) -> Module<T> {
        match *self {
            Module(ref __self_0_0) => Module(::core::clone::Clone::clone(&(*__self_0_0))),
        }
    }
}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<T: ::core::marker::Copy + Config> ::core::marker::Copy for Module<T> {}
impl<T: Config> ::core::marker::StructuralPartialEq for Module<T> {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<T: ::core::cmp::PartialEq + Config> ::core::cmp::PartialEq for Module<T> {
    #[inline]
    fn eq(&self, other: &Module<T>) -> bool {
        match *other {
            Module(ref __self_1_0) => match *self {
                Module(ref __self_0_0) => (*__self_0_0) == (*__self_1_0),
            },
        }
    }
    #[inline]
    fn ne(&self, other: &Module<T>) -> bool {
        match *other {
            Module(ref __self_1_0) => match *self {
                Module(ref __self_0_0) => (*__self_0_0) != (*__self_1_0),
            },
        }
    }
}
impl<T: Config> ::core::marker::StructuralEq for Module<T> {}
#[automatically_derived]
#[allow(unused_qualifications)]
impl<T: ::core::cmp::Eq + Config> ::core::cmp::Eq for Module<T> {
    #[inline]
    #[doc(hidden)]
    #[no_coverage]
    fn assert_receiver_is_total_eq(&self) -> () {
        {
            let _: ::core::cmp::AssertParamIsEq<
                ::frame_support::sp_std::marker::PhantomData<(T,)>,
            >;
        }
    }
}
impl<T: Config> core::fmt::Debug for Module<T>
where
    T: core::fmt::Debug,
{
    fn fmt(&self, fmt: &mut core::fmt::Formatter) -> core::fmt::Result {
        fmt.debug_tuple("Module").field(&self.0).finish()
    }
}
#[doc = " Type alias to `Module`, to be used by `construct_runtime`."]
#[allow(dead_code)]
pub type Pallet<T> = Module<T>;
impl<T: frame_system::Config + Config>
    ::frame_support::traits::OnInitialize<<T as frame_system::Config>::BlockNumber> for Module<T>
{
    fn on_initialize(now: T::BlockNumber) -> Weight {
        let __within_span__ = {
            use ::tracing::__macro_support::Callsite as _;
            static CALLSITE: ::tracing::__macro_support::MacroCallsite = {
                use ::tracing::__macro_support::MacroCallsite;
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "on_initialize",
                        "crml_eth_state_oracle",
                        ::tracing::Level::TRACE,
                        Some("crml/eth-state-oracle/src/lib.rs"),
                        Some(128u32),
                        Some("crml_eth_state_oracle"),
                        ::tracing_core::field::FieldSet::new(
                            &[],
                            ::tracing_core::callsite::Identifier(&CALLSITE),
                        ),
                        ::tracing::metadata::Kind::SPAN,
                    )
                };
                MacroCallsite::new(&META)
            };
            let mut interest = ::tracing::subscriber::Interest::never();
            if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                && {
                    interest = CALLSITE.interest();
                    !interest.is_never()
                }
                && CALLSITE.is_enabled(interest)
            {
                let meta = CALLSITE.metadata();
                ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
            } else {
                let span = CALLSITE.disabled_span();
                {};
                span
            }
        };
        let __tracing_guard__ = __within_span__.enter();
        {
            let mut consumed_weight = DbWeight::get().reads(1);
            if ResponsesValidAtBlock::<T>::contains_key(now) {
                for call_request_id in ResponsesValidAtBlock::<T>::take(now) {
                    ResponsesForCallback::append(call_request_id);
                    consumed_weight += DbWeight::get().writes(1);
                }
            }
            consumed_weight
        }
    }
}
impl<T: Config> ::frame_support::traits::OnRuntimeUpgrade for Module<T> {
    fn on_runtime_upgrade() -> ::frame_support::dispatch::Weight {
        let __within_span__ = {
            use ::tracing::__macro_support::Callsite as _;
            static CALLSITE: ::tracing::__macro_support::MacroCallsite = {
                use ::tracing::__macro_support::MacroCallsite;
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "on_runtime_upgrade",
                        "crml_eth_state_oracle",
                        ::tracing::Level::TRACE,
                        Some("crml/eth-state-oracle/src/lib.rs"),
                        Some(128u32),
                        Some("crml_eth_state_oracle"),
                        ::tracing_core::field::FieldSet::new(
                            &[],
                            ::tracing_core::callsite::Identifier(&CALLSITE),
                        ),
                        ::tracing::metadata::Kind::SPAN,
                    )
                };
                MacroCallsite::new(&META)
            };
            let mut interest = ::tracing::subscriber::Interest::never();
            if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                && {
                    interest = CALLSITE.interest();
                    !interest.is_never()
                }
                && CALLSITE.is_enabled(interest)
            {
                let meta = CALLSITE.metadata();
                ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
            } else {
                let span = CALLSITE.disabled_span();
                {};
                span
            }
        };
        let __tracing_guard__ = __within_span__.enter();
        let pallet_name = < < T as frame_system :: Config > :: PalletInfo as :: frame_support :: traits :: PalletInfo > :: name :: < Self > () . unwrap_or ("<unknown pallet name>") ;
        {
            let lvl = ::log::Level::Info;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    ::core::fmt::Arguments::new_v1(
                        &["\u{2705} no migration for "],
                        &match (&pallet_name,) {
                            (arg0,) => [::core::fmt::ArgumentV1::new(
                                arg0,
                                ::core::fmt::Display::fmt,
                            )],
                        },
                    ),
                    lvl,
                    &(
                        ::frame_support::LOG_TARGET,
                        "crml_eth_state_oracle",
                        "crml/eth-state-oracle/src/lib.rs",
                        128u32,
                    ),
                );
            }
        };
        0
    }
}
impl<T: frame_system::Config + Config>
    ::frame_support::traits::OnFinalize<<T as frame_system::Config>::BlockNumber> for Module<T>
{
}
impl<T: frame_system::Config + Config>
    ::frame_support::traits::OnIdle<<T as frame_system::Config>::BlockNumber> for Module<T>
{
    fn on_idle(_now: T::BlockNumber, remaining_weight: Weight) -> Weight {
        let __within_span__ = {
            use ::tracing::__macro_support::Callsite as _;
            static CALLSITE: ::tracing::__macro_support::MacroCallsite = {
                use ::tracing::__macro_support::MacroCallsite;
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "on_idle",
                        "crml_eth_state_oracle",
                        ::tracing::Level::TRACE,
                        Some("crml/eth-state-oracle/src/lib.rs"),
                        Some(128u32),
                        Some("crml_eth_state_oracle"),
                        ::tracing_core::field::FieldSet::new(
                            &[],
                            ::tracing_core::callsite::Identifier(&CALLSITE),
                        ),
                        ::tracing::metadata::Kind::SPAN,
                    )
                };
                MacroCallsite::new(&META)
            };
            let mut interest = ::tracing::subscriber::Interest::never();
            if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                && {
                    interest = CALLSITE.interest();
                    !interest.is_never()
                }
                && CALLSITE.is_enabled(interest)
            {
                let meta = CALLSITE.metadata();
                ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
            } else {
                let span = CALLSITE.disabled_span();
                {};
                span
            }
        };
        let __tracing_guard__ = __within_span__.enter();
        {
            if ResponsesForCallback::decode_len().unwrap_or(0).is_zero() {
                return DbWeight::get().reads(1);
            }
            let mut consumed_weight = DbWeight::get().reads(2) + DbWeight::get().writes(2);
            let mut callbacks = ResponsesForCallback::get();
            for call_request_id in callbacks.drain(..) {
                if ResponsesChallenged::<T>::contains_key(call_request_id) {
                    continue;
                }
                let next_weight = DbWeight::get().writes(5) + DbWeight::get().reads(6);
                if consumed_weight + next_weight > remaining_weight {
                    return consumed_weight;
                }
                let request = Requests::take(call_request_id).unwrap();
                let response = Responses::<T>::take(call_request_id).unwrap();
                let return_data = ResponseReturnData::take(call_request_id);
                RequestInputData::remove(call_request_id);
                let execution_info = Self::try_callback(
                    call_request_id,
                    &request,
                    &response.reporter,
                    return_data.as_ref(),
                );
                consumed_weight += execution_info.consumed_weight();
                if let Some(error) = execution_info.error() {
                    Self::deposit_event(Event::CallbackErr(call_request_id, error));
                } else {
                    Self::deposit_event(Event::Callback(
                        call_request_id,
                        execution_info.consumed_weight(),
                    ));
                }
            }
            ResponsesForCallback::put(callbacks);
            consumed_weight
        }
    }
}
impl<T: frame_system::Config + Config>
    ::frame_support::traits::OffchainWorker<<T as frame_system::Config>::BlockNumber>
    for Module<T>
{
}
impl<T: Config> Module<T> {
    #[doc = " Deposits an event using `frame_system::Pallet::deposit_event`."]
    fn deposit_event(event: impl Into<<T as Config>::Event>) {
        <frame_system::Pallet<T>>::deposit_event(event.into())
    }
}
#[cfg(feature = "std")]
impl<T: Config> ::frame_support::traits::IntegrityTest for Module<T> {}
#[doc = " Can also be called using [`Call`]."]
#[doc = ""]
#[doc = " [`Call`]: enum.Call.html"]
impl<T: Config> Module<T> {
    #[allow(unreachable_code)]
    #[doc = r" Submit response for a a remote call request"]
    #[doc = r" `return_data` - the rlp encoded output of the remote call"]
    #[doc = r" `eth_block_number` - the ethereum block number the request was made"]
    #[doc = r""]
    #[doc = r" Caller should be a configured relayer (i.e. authorized or staked)"]
    #[doc = r" Only accepts the first response for a given request"]
    #[doc = r""]
    #[doc = r""]
    #[doc = r" NOTE: Calling this function will bypass origin filters."]
    pub fn submit_call_response(
        origin: T::Origin,
        request_id: RequestId,
        return_data: Vec<u8>,
        eth_block_number: u64,
    ) -> ::frame_support::dispatch::DispatchResult {
        let __within_span__ = {
            use ::tracing::__macro_support::Callsite as _;
            static CALLSITE: ::tracing::__macro_support::MacroCallsite = {
                use ::tracing::__macro_support::MacroCallsite;
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "submit_call_response",
                        "crml_eth_state_oracle",
                        ::tracing::Level::TRACE,
                        Some("crml/eth-state-oracle/src/lib.rs"),
                        Some(128u32),
                        Some("crml_eth_state_oracle"),
                        ::tracing_core::field::FieldSet::new(
                            &[],
                            ::tracing_core::callsite::Identifier(&CALLSITE),
                        ),
                        ::tracing::metadata::Kind::SPAN,
                    )
                };
                MacroCallsite::new(&META)
            };
            let mut interest = ::tracing::subscriber::Interest::never();
            if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                && {
                    interest = CALLSITE.interest();
                    !interest.is_never()
                }
                && CALLSITE.is_enabled(interest)
            {
                let meta = CALLSITE.metadata();
                ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
            } else {
                let span = CALLSITE.disabled_span();
                {};
                span
            }
        };
        let __tracing_guard__ = __within_span__.enter();
        {
            let origin = ensure_signed(origin)?;
            {
                if !Requests::contains_key(request_id) {
                    {
                        return Err(Error::<T>::NoRequest.into());
                    };
                }
            };
            {
                if !!<Responses<T>>::contains_key(request_id) {
                    {
                        return Err(Error::<T>::ResponseExists.into());
                    };
                }
            };
            let response = CallResponse {
                return_data_digest: sp_io::hashing::blake2_256(return_data.as_slice()),
                eth_block_number,
                reporter: origin,
            };
            <Responses<T>>::insert(request_id, response);
            ResponseReturnData::insert(request_id, return_data);
            let execute_block =
                <frame_system::Pallet<T>>::block_number() + T::ChallengePeriod::get();
            <ResponsesValidAtBlock<T>>::append(execute_block, request_id);
        }
        Ok(())
    }
    #[allow(unreachable_code)]
    #[doc = r" Initiate a challenge on the current response for `request_id`"]
    #[doc = r" Valid challenge scenarios are:"]
    #[doc = r" - incorrect value"]
    #[doc = r" - The block number of the response is stale or from the future"]
    #[doc = r"   request.timestamp - lenience > block.timestamp > request.timestamp + lenience"]
    #[doc = r""]
    #[doc = r" NOTE: Calling this function will bypass origin filters."]
    pub fn submit_response_challenge(
        origin: T::Origin,
        request_id: RequestId,
    ) -> ::frame_support::dispatch::DispatchResult {
        let __within_span__ = {
            use ::tracing::__macro_support::Callsite as _;
            static CALLSITE: ::tracing::__macro_support::MacroCallsite = {
                use ::tracing::__macro_support::MacroCallsite;
                static META: ::tracing::Metadata<'static> = {
                    ::tracing_core::metadata::Metadata::new(
                        "submit_response_challenge",
                        "crml_eth_state_oracle",
                        ::tracing::Level::TRACE,
                        Some("crml/eth-state-oracle/src/lib.rs"),
                        Some(128u32),
                        Some("crml_eth_state_oracle"),
                        ::tracing_core::field::FieldSet::new(
                            &[],
                            ::tracing_core::callsite::Identifier(&CALLSITE),
                        ),
                        ::tracing::metadata::Kind::SPAN,
                    )
                };
                MacroCallsite::new(&META)
            };
            let mut interest = ::tracing::subscriber::Interest::never();
            if ::tracing::Level::TRACE <= ::tracing::level_filters::STATIC_MAX_LEVEL
                && ::tracing::Level::TRACE <= ::tracing::level_filters::LevelFilter::current()
                && {
                    interest = CALLSITE.interest();
                    !interest.is_never()
                }
                && CALLSITE.is_enabled(interest)
            {
                let meta = CALLSITE.metadata();
                ::tracing::Span::new(meta, &{ meta.fields().value_set(&[]) })
            } else {
                let span = CALLSITE.disabled_span();
                {};
                span
            }
        };
        let __tracing_guard__ = __within_span__.enter();
        {
            let origin = ensure_signed(origin)?;
            {
                if !Requests::contains_key(request_id) {
                    {
                        return Err(Error::<T>::NoRequest.into());
                    };
                }
            };
            {
                if !!ResponsesChallenged::<T>::contains_key(request_id) {
                    {
                        return Err(Error::<T>::DuplicateChallenge.into());
                    };
                }
            };
            if let Some(response) = Responses::<T>::get(request_id) {
                let request = Requests::get(request_id).unwrap();
                let request_input = RequestInputData::get(request_id);
                let challenge_subscription_id = T::EthCallOracle::call_at(
                    request.destination,
                    request_input.as_ref(),
                    request.timestamp,
                );
                ResponsesChallenged::<T>::insert(request_id, origin);
                ChallengeSubscriptions::insert(challenge_subscription_id, request_id);
            } else {
                return Err(Error::<T>::NoResponse.into());
            }
        }
        Ok(())
    }
}
#[doc = " Dispatchable calls."]
#[doc = ""]
#[doc = " Each variant of this enum maps to a dispatchable function from the associated module."]
#[scale_info(skip_type_params(T), capture_docs = "always")]
pub enum Call<T: Config> {
    #[doc(hidden)]
    #[codec(skip)]
    __PhantomItem(
        ::frame_support::sp_std::marker::PhantomData<(T,)>,
        ::frame_support::Never,
    ),
    #[allow(non_camel_case_types)]
    #[doc = r" Submit response for a a remote call request"]
    #[doc = r" `return_data` - the rlp encoded output of the remote call"]
    #[doc = r" `eth_block_number` - the ethereum block number the request was made"]
    #[doc = r""]
    #[doc = r" Caller should be a configured relayer (i.e. authorized or staked)"]
    #[doc = r" Only accepts the first response for a given request"]
    #[doc = r""]
    submit_call_response {
        request_id: RequestId,
        return_data: Vec<u8>,
        eth_block_number: u64,
    },
    #[allow(non_camel_case_types)]
    #[doc = r" Initiate a challenge on the current response for `request_id`"]
    #[doc = r" Valid challenge scenarios are:"]
    #[doc = r" - incorrect value"]
    #[doc = r" - The block number of the response is stale or from the future"]
    #[doc = r"   request.timestamp - lenience > block.timestamp > request.timestamp + lenience"]
    submit_response_challenge { request_id: RequestId },
}
const _: () = {
    impl<T: Config> ::codec::Encode for Call<T> {
        fn encode_to<__CodecOutputEdqy: ::codec::Output + ?::core::marker::Sized>(
            &self,
            __codec_dest_edqy: &mut __CodecOutputEdqy,
        ) {
            match *self {
                Call::submit_call_response {
                    ref request_id,
                    ref return_data,
                    ref eth_block_number,
                } => {
                    __codec_dest_edqy.push_byte(0usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(request_id, __codec_dest_edqy);
                    ::codec::Encode::encode_to(return_data, __codec_dest_edqy);
                    ::codec::Encode::encode_to(eth_block_number, __codec_dest_edqy);
                }
                Call::submit_response_challenge { ref request_id } => {
                    __codec_dest_edqy.push_byte(1usize as ::core::primitive::u8);
                    ::codec::Encode::encode_to(request_id, __codec_dest_edqy);
                }
                _ => (),
            }
        }
    }
    impl<T: Config> ::codec::EncodeLike for Call<T> {}
};
const _: () = {
    impl<T: Config> ::codec::Decode for Call<T> {
        fn decode<__CodecInputEdqy: ::codec::Input>(
            __codec_input_edqy: &mut __CodecInputEdqy,
        ) -> ::core::result::Result<Self, ::codec::Error> {
            match __codec_input_edqy
                .read_byte()
                .map_err(|e| e.chain("Could not decode `Call`, failed to read variant byte"))?
            {
                __codec_x_edqy if __codec_x_edqy == 0usize as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(Call::<T>::submit_call_response {
                        request_id: {
                            let __codec_res_edqy =
                                <RequestId as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy {
                                ::core::result::Result::Err(e) => {
                                    return ::core::result::Result::Err(e.chain(
                                        "Could not decode `Call::submit_call_response::request_id`",
                                    ))
                                }
                                ::core::result::Result::Ok(__codec_res_edqy) => __codec_res_edqy,
                            }
                        },
                        return_data: {
                            let __codec_res_edqy =
                                <Vec<u8> as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy { :: core :: result :: Result :: Err (e) => return :: core :: result :: Result :: Err (e . chain ("Could not decode `Call::submit_call_response::return_data`")) , :: core :: result :: Result :: Ok (__codec_res_edqy) => __codec_res_edqy , }
                        },
                        eth_block_number: {
                            let __codec_res_edqy =
                                <u64 as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy { :: core :: result :: Result :: Err (e) => return :: core :: result :: Result :: Err (e . chain ("Could not decode `Call::submit_call_response::eth_block_number`")) , :: core :: result :: Result :: Ok (__codec_res_edqy) => __codec_res_edqy , }
                        },
                    })
                }
                __codec_x_edqy if __codec_x_edqy == 1usize as ::core::primitive::u8 => {
                    ::core::result::Result::Ok(Call::<T>::submit_response_challenge {
                        request_id: {
                            let __codec_res_edqy =
                                <RequestId as ::codec::Decode>::decode(__codec_input_edqy);
                            match __codec_res_edqy { :: core :: result :: Result :: Err (e) => return :: core :: result :: Result :: Err (e . chain ("Could not decode `Call::submit_response_challenge::request_id`")) , :: core :: result :: Result :: Ok (__codec_res_edqy) => __codec_res_edqy , }
                        },
                    })
                }
                _ => ::core::result::Result::Err(<_ as ::core::convert::Into<_>>::into(
                    "Could not decode `Call`, variant doesn\'t exist",
                )),
            }
        }
    }
};
#[allow(non_upper_case_globals, unused_attributes, unused_qualifications)]
const _: () = {
    impl<T: Config> ::scale_info::TypeInfo for Call<T>
    where
        ::frame_support::sp_std::marker::PhantomData<(T,)>: ::scale_info::TypeInfo + 'static,
        T: Config + 'static,
    {
        type Identity = Self;
        fn type_info() -> ::scale_info::Type {
            :: scale_info :: Type :: builder () . path (:: scale_info :: Path :: new ("Call" , "crml_eth_state_oracle")) . type_params (< [_] > :: into_vec (box [:: scale_info :: TypeParameter :: new ("T" , :: core :: option :: Option :: None)])) . docs_always (& ["Dispatchable calls." , "" , "Each variant of this enum maps to a dispatchable function from the associated module."]) . variant (:: scale_info :: build :: Variants :: new () . variant ("submit_call_response" , | v | v . index (0usize as :: core :: primitive :: u8) . fields (:: scale_info :: build :: Fields :: named () . field (| f | f . ty :: < RequestId > () . name ("request_id") . type_name ("RequestId") . docs_always (& [])) . field (| f | f . ty :: < Vec < u8 > > () . name ("return_data") . type_name ("Vec<u8>") . docs_always (& [])) . field (| f | f . ty :: < u64 > () . name ("eth_block_number") . type_name ("u64") . docs_always (& []))) . docs_always (& ["Submit response for a a remote call request" , "`return_data` - the rlp encoded output of the remote call" , "`eth_block_number` - the ethereum block number the request was made" , "" , "Caller should be a configured relayer (i.e. authorized or staked)" , "Only accepts the first response for a given request" , ""])) . variant ("submit_response_challenge" , | v | v . index (1usize as :: core :: primitive :: u8) . fields (:: scale_info :: build :: Fields :: named () . field (| f | f . ty :: < RequestId > () . name ("request_id") . type_name ("RequestId") . docs_always (& []))) . docs_always (& ["Initiate a challenge on the current response for `request_id`" , "Valid challenge scenarios are:" , "- incorrect value" , "- The block number of the response is stale or from the future" , "  request.timestamp - lenience > block.timestamp > request.timestamp + lenience"])))
        }
    };
};
impl<T: Config> Call<T> {
    #[doc = "Create a call with the variant `submit_call_response`."]
    pub fn new_call_variant_submit_call_response(
        request_id: RequestId,
        return_data: Vec<u8>,
        eth_block_number: u64,
    ) -> Self {
        Self::submit_call_response {
            request_id,
            return_data,
            eth_block_number,
        }
    }
    #[doc = "Create a call with the variant `submit_response_challenge`."]
    pub fn new_call_variant_submit_response_challenge(request_id: RequestId) -> Self {
        Self::submit_response_challenge { request_id }
    }
}
impl<T: Config> ::frame_support::traits::GetStorageVersion for Module<T> {
    fn current_storage_version() -> ::frame_support::traits::StorageVersion {
        Default::default()
    }
    fn on_chain_storage_version() -> ::frame_support::traits::StorageVersion {
        ::frame_support::traits::StorageVersion::get::<Self>()
    }
}
impl<T: Config> ::frame_support::dispatch::GetDispatchInfo for Call<T> {
    fn get_dispatch_info(&self) -> ::frame_support::dispatch::DispatchInfo {
        match *self {
            Call::submit_call_response {
                ref request_id,
                ref return_data,
                ref eth_block_number,
            } => {
                let __pallet_base_weight = 500_000;
                let __pallet_weight = <dyn ::frame_support::dispatch::WeighData<(
                    &RequestId,
                    &Vec<u8>,
                    &u64,
                )>>::weigh_data(
                    &__pallet_base_weight,
                    (request_id, return_data, eth_block_number),
                );
                let __pallet_class = <dyn ::frame_support::dispatch::ClassifyDispatch<(
                    &RequestId,
                    &Vec<u8>,
                    &u64,
                )>>::classify_dispatch(
                    &__pallet_base_weight,
                    (request_id, return_data, eth_block_number),
                );
                let __pallet_pays_fee = <dyn ::frame_support::dispatch::PaysFee<(
                    &RequestId,
                    &Vec<u8>,
                    &u64,
                )>>::pays_fee(
                    &__pallet_base_weight,
                    (request_id, return_data, eth_block_number),
                );
                ::frame_support::dispatch::DispatchInfo {
                    weight: __pallet_weight,
                    class: __pallet_class,
                    pays_fee: __pallet_pays_fee,
                }
            }
            Call::submit_response_challenge { ref request_id } => {
                let __pallet_base_weight = 500_000;
                let __pallet_weight =
                    <dyn ::frame_support::dispatch::WeighData<(&RequestId,)>>::weigh_data(
                        &__pallet_base_weight,
                        (request_id,),
                    );
                let __pallet_class = < dyn :: frame_support :: dispatch :: ClassifyDispatch < (& RequestId ,) > > :: classify_dispatch (& __pallet_base_weight , (request_id ,)) ;
                let __pallet_pays_fee =
                    <dyn ::frame_support::dispatch::PaysFee<(&RequestId,)>>::pays_fee(
                        &__pallet_base_weight,
                        (request_id,),
                    );
                ::frame_support::dispatch::DispatchInfo {
                    weight: __pallet_weight,
                    class: __pallet_class,
                    pays_fee: __pallet_pays_fee,
                }
            }
            Call::__PhantomItem(_, _) => {
                ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                    &["internal error: entered unreachable code: "],
                    &match (&"__PhantomItem should never be used.",) {
                        (arg0,) => [::core::fmt::ArgumentV1::new(
                            arg0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                ))
            }
        }
    }
}
impl<T: Config> ::frame_support::traits::PalletInfoAccess for Module<T> {
    fn index() -> usize {
        <<T as frame_system::Config>::PalletInfo as ::frame_support::traits::PalletInfo>::index::<
            Self,
        >()
        .expect(
            "Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime",
        )
    }
    fn name() -> &'static str {
        < < T as frame_system :: Config > :: PalletInfo as :: frame_support :: traits :: PalletInfo > :: name :: < Self > () . expect ("Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime")
    }
    fn module_name() -> &'static str {
        < < T as frame_system :: Config > :: PalletInfo as :: frame_support :: traits :: PalletInfo > :: module_name :: < Self > () . expect ("Pallet is part of the runtime because pallet `Config` trait is \
						implemented by the runtime")
    }
    fn crate_version() -> ::frame_support::traits::CrateVersion {
        frame_support::traits::CrateVersion {
            major: 1u16,
            minor: 0u8,
            patch: 0u8,
        }
    }
}
impl<T: Config> ::frame_support::traits::PalletsInfoAccess for Module<T> {
    fn count() -> usize {
        1
    }
    fn accumulate(
        acc: &mut ::frame_support::sp_std::vec::Vec<::frame_support::traits::PalletInfoData>,
    ) {
        use ::frame_support::traits::PalletInfoAccess;
        let item = ::frame_support::traits::PalletInfoData {
            index: Self::index(),
            name: Self::name(),
            module_name: Self::module_name(),
            crate_version: Self::crate_version(),
        };
        acc.push(item);
    }
}
impl<T: Config> ::frame_support::dispatch::GetCallName for Call<T> {
    fn get_call_name(&self) -> &'static str {
        match *self {
            Call::submit_call_response {
                ref request_id,
                ref return_data,
                ref eth_block_number,
            } => {
                let _ = (request_id, return_data, eth_block_number);
                "submit_call_response"
            }
            Call::submit_response_challenge { ref request_id } => {
                let _ = (request_id);
                "submit_response_challenge"
            }
            Call::__PhantomItem(_, _) => {
                ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                    &["internal error: entered unreachable code: "],
                    &match (&"__PhantomItem should never be used.",) {
                        (arg0,) => [::core::fmt::ArgumentV1::new(
                            arg0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                ))
            }
        }
    }
    fn get_call_names() -> &'static [&'static str] {
        &["submit_call_response", "submit_response_challenge"]
    }
}
impl<T: Config> ::frame_support::traits::OnGenesis for Module<T> {
    fn on_genesis() {
        let storage_version =
            <Self as ::frame_support::traits::GetStorageVersion>::current_storage_version();
        storage_version.put::<Self>();
    }
}
impl<T: Config> ::frame_support::dispatch::Clone for Call<T> {
    fn clone(&self) -> Self {
        match *self {
            Call::submit_call_response {
                ref request_id,
                ref return_data,
                ref eth_block_number,
            } => Call::submit_call_response {
                request_id: (*request_id).clone(),
                return_data: (*return_data).clone(),
                eth_block_number: (*eth_block_number).clone(),
            },
            Call::submit_response_challenge { ref request_id } => Call::submit_response_challenge {
                request_id: (*request_id).clone(),
            },
            _ => ::core::panicking::panic("internal error: entered unreachable code"),
        }
    }
}
impl<T: Config> ::frame_support::dispatch::PartialEq for Call<T> {
    fn eq(&self, _other: &Self) -> bool {
        match *self {
            Call::submit_call_response {
                ref request_id,
                ref return_data,
                ref eth_block_number,
            } => {
                let self_params = (request_id, return_data, eth_block_number);
                if let Call::submit_call_response {
                    ref request_id,
                    ref return_data,
                    ref eth_block_number,
                } = *_other
                {
                    self_params == (request_id, return_data, eth_block_number)
                } else {
                    match *_other {
                        Call::__PhantomItem(_, _) => {
                            ::core::panicking::panic("internal error: entered unreachable code")
                        }
                        _ => false,
                    }
                }
            }
            Call::submit_response_challenge { ref request_id } => {
                let self_params = (request_id,);
                if let Call::submit_response_challenge { ref request_id } = *_other {
                    self_params == (request_id,)
                } else {
                    match *_other {
                        Call::__PhantomItem(_, _) => {
                            ::core::panicking::panic("internal error: entered unreachable code")
                        }
                        _ => false,
                    }
                }
            }
            _ => ::core::panicking::panic("internal error: entered unreachable code"),
        }
    }
}
impl<T: Config> ::frame_support::dispatch::Eq for Call<T> {}
impl<T: Config> ::frame_support::dispatch::fmt::Debug for Call<T> {
    fn fmt(
        &self,
        _f: &mut ::frame_support::dispatch::fmt::Formatter,
    ) -> ::frame_support::dispatch::result::Result<(), ::frame_support::dispatch::fmt::Error> {
        match *self {
            Call::submit_call_response {
                ref request_id,
                ref return_data,
                ref eth_block_number,
            } => _f.write_fmt(::core::fmt::Arguments::new_v1(
                &["", ""],
                &match (
                    &"submit_call_response",
                    &(
                        request_id.clone(),
                        return_data.clone(),
                        eth_block_number.clone(),
                    ),
                ) {
                    (arg0, arg1) => [
                        ::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Display::fmt),
                        ::core::fmt::ArgumentV1::new(arg1, ::core::fmt::Debug::fmt),
                    ],
                },
            )),
            Call::submit_response_challenge { ref request_id } => {
                _f.write_fmt(::core::fmt::Arguments::new_v1(
                    &["", ""],
                    &match (&"submit_response_challenge", &(request_id.clone(),)) {
                        (arg0, arg1) => [
                            ::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Display::fmt),
                            ::core::fmt::ArgumentV1::new(arg1, ::core::fmt::Debug::fmt),
                        ],
                    },
                ))
            }
            _ => ::core::panicking::panic("internal error: entered unreachable code"),
        }
    }
}
impl<T: Config> ::frame_support::traits::UnfilteredDispatchable for Call<T> {
    type Origin = T::Origin;
    fn dispatch_bypass_filter(
        self,
        _origin: Self::Origin,
    ) -> ::frame_support::dispatch::DispatchResultWithPostInfo {
        match self {
            Call::submit_call_response {
                request_id,
                return_data,
                eth_block_number,
            } => <Module<T>>::submit_call_response(
                _origin,
                request_id,
                return_data,
                eth_block_number,
            )
            .map(Into::into)
            .map_err(Into::into),
            Call::submit_response_challenge { request_id } => {
                <Module<T>>::submit_response_challenge(_origin, request_id)
                    .map(Into::into)
                    .map_err(Into::into)
            }
            Call::__PhantomItem(_, _) => {
                ::core::panicking::panic_fmt(::core::fmt::Arguments::new_v1(
                    &["internal error: entered unreachable code: "],
                    &match (&"__PhantomItem should never be used.",) {
                        (arg0,) => [::core::fmt::ArgumentV1::new(
                            arg0,
                            ::core::fmt::Display::fmt,
                        )],
                    },
                ))
            }
        }
    }
}
impl<T: Config> ::frame_support::dispatch::Callable<T> for Module<T> {
    type Call = Call<T>;
}
impl<T: Config> Module<T> {
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn call_functions() -> ::frame_support::metadata::PalletCallMetadata {
        ::frame_support::scale_info::meta_type::<Call<T>>().into()
    }
}
impl<T: Config> Module<T> {
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn error_metadata() -> Option<::frame_support::metadata::PalletErrorMetadata> {
        None
    }
}
impl<T: 'static + Config> Module<T> {
    #[doc(hidden)]
    #[allow(dead_code)]
    pub fn pallet_constants_metadata(
    ) -> ::frame_support::sp_std::vec::Vec<::frame_support::metadata::PalletConstantMetadata> {
        ::alloc::vec::Vec::new()
    }
}
#[doc(hidden)]
pub mod __substrate_genesis_config_check {
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_genesis_config_defined;
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_std_enabled_for_genesis;
}
#[doc(hidden)]
pub mod __substrate_event_check {
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_event_part_defined;
}
#[doc(hidden)]
pub mod __substrate_inherent_check {
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_inherent_part_defined;
}
#[doc(hidden)]
pub mod __substrate_validate_unsigned_check {
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_validate_unsigned_part_defined;
}
#[doc(hidden)]
pub mod __substrate_call_check {
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_call_part_defined;
}
#[doc(hidden)]
pub mod __substrate_origin_check {
    #[doc(hidden)]
    pub use __dummy_part_checker_0 as is_origin_part_defined;
}
impl<T: Config> EthCallOracleSubscriber for Module<T> {
    type CallId = u64;
    #[doc = " Compare response from relayer with response from validators"]
    #[doc = " Either the challenger will be slashed or"]
    fn on_call_at_complete(
        call_id: Self::CallId,
        validator_return_data: &[u8],
        block_number: u64,
        block_timestamp: u64,
    ) {
        if let Some(request_id) = ChallengeSubscriptions::get(call_id) {
            if let Some(challenger) = ResponsesChallenged::<T>::get(request_id) {
                let reported_return_data = ResponseReturnData::get(request_id);
                if reported_return_data != validator_return_data {}
                let request_timestamp = Requests::get(request_id).unwrap().timestamp;
                let LENIENCE = 15_u64;
                let is_stale = block_timestamp < (request_timestamp - LENIENCE);
                if is_stale {}
                let is_future = block_timestamp > (request_timestamp + LENIENCE);
                if is_future {}
            }
        }
    }
}
impl<T: Config> Module<T> {
    #[doc = " Try to execute a callback"]
    #[doc = " `request` - the original request"]
    #[doc = " `relayer` - the address of the relayer"]
    #[doc = " `return_data` - the returndata of the request (fulfilled by the relayer)"]
    fn try_callback(
        request_id: RequestId,
        request: &CallRequest,
        relayer: &T::AccountId,
        return_data: &[u8],
    ) -> <<T as Config>::ContractExecutor as ContractExecutor>::Result {
        let caller_ss58_address = T::AddressMapping::into_account_id(request.caller);
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    ::core::fmt::Arguments::new_v1(
                        &[
                            "\u{1f52e} preparing callback for: ",
                            ", caller: ",
                            ", caller(ss58): ",
                        ],
                        &match (&request_id, &request.caller, &caller_ss58_address) {
                            (arg0, arg1, arg2) => [
                                ::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Debug::fmt),
                                ::core::fmt::ArgumentV1::new(arg1, ::core::fmt::Debug::fmt),
                                ::core::fmt::ArgumentV1::new(arg2, ::core::fmt::Debug::fmt),
                            ],
                        },
                    ),
                    lvl,
                    &(
                        crate::LOG_TARGET,
                        "crml_eth_state_oracle",
                        "crml/eth-state-oracle/src/lib.rs",
                        339u32,
                    ),
                );
            }
        };
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    ::core::fmt::Arguments::new_v1(
                        &["\u{1f52e} paying bounty for callback(", "), bounty: "],
                        &match (&request_id, &request.bounty) {
                            (arg0, arg1) => [
                                ::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Debug::fmt),
                                ::core::fmt::ArgumentV1::new(arg1, ::core::fmt::Debug::fmt),
                            ],
                        },
                    ),
                    lvl,
                    &(
                        crate::LOG_TARGET,
                        "crml_eth_state_oracle",
                        "crml/eth-state-oracle/src/lib.rs",
                        348u32,
                    ),
                );
            }
        };
        let _ = T::MultiCurrency::transfer(
            &caller_ss58_address,
            relayer,
            T::MultiCurrency::fee_currency(),
            request.bounty,
            ExistenceRequirement::AllowDeath,
        )
        .map_err(|_| {
            return DispatchErrorWithPostInfo {
                post_info: PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                },
                error: DispatchError::Module {
                    index: 27_u8,
                    error: 5,
                    message: None,
                },
            };
        });
        let state_oracle_precompile = T::StateOraclePrecompileAddress::get();
        let state_oracle_precompile_ss58_address =
            T::AddressMapping::into_account_id(state_oracle_precompile);
        let max_fee_per_gas = U256::from(T::MinGasPrice::get());
        let max_priority_fee_per_gas = U256::zero();
        let total_fee: Balance = ((max_fee_per_gas * request.callback_gas_limit)
            / U256::from(100_000_000_000_000_u64)
            + max_priority_fee_per_gas)
            .saturated_into();
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    ::core::fmt::Arguments::new_v1(
                        &[
                            "\u{1f52e} required gas fee for callback(",
                            "), total_fee: ",
                            ", gas_limit: ",
                        ],
                        &match (&request_id, &total_fee, &request.callback_gas_limit) {
                            (arg0, arg1, arg2) => [
                                ::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Debug::fmt),
                                ::core::fmt::ArgumentV1::new(arg1, ::core::fmt::Debug::fmt),
                                ::core::fmt::ArgumentV1::new(arg2, ::core::fmt::Debug::fmt),
                            ],
                        },
                    ),
                    lvl,
                    &(
                        crate::LOG_TARGET,
                        "crml_eth_state_oracle",
                        "crml/eth-state-oracle/src/lib.rs",
                        391u32,
                    ),
                );
            }
        };
        let _ = T::MultiCurrency::transfer(
            &caller_ss58_address,
            &state_oracle_precompile_ss58_address,
            T::MultiCurrency::fee_currency(),
            total_fee,
            ExistenceRequirement::AllowDeath,
        )
        .map_err(|_| {
            return DispatchErrorWithPostInfo {
                post_info: PostDispatchInfo {
                    actual_weight: None,
                    pays_fee: Pays::No,
                },
                error: DispatchError::Module {
                    index: 27_u8,
                    error: 4,
                    message: None,
                },
            };
        });
        let mut return_data_ = return_data.to_vec();
        return_data_.resize(32, 0);
        let callback_input = [
            request.callback_signature.as_ref(),
            EthAbiCodec::encode(&request_id).as_ref(),
            return_data_.as_ref(),
        ]
        .concat();
        {
            let lvl = ::log::Level::Debug;
            if lvl <= ::log::STATIC_MAX_LEVEL && lvl <= ::log::max_level() {
                ::log::__private_api_log(
                    ::core::fmt::Arguments::new_v1(
                        &["\u{1f52e} execute callback ", ", input: "],
                        &match (&request_id, &callback_input) {
                            (arg0, arg1) => [
                                ::core::fmt::ArgumentV1::new(arg0, ::core::fmt::Debug::fmt),
                                ::core::fmt::ArgumentV1::new(arg1, ::core::fmt::Debug::fmt),
                            ],
                        },
                    ),
                    lvl,
                    &(
                        crate::LOG_TARGET,
                        "crml_eth_state_oracle",
                        "crml/eth-state-oracle/src/lib.rs",
                        431u32,
                    ),
                );
            }
        };
        T::ContractExecutor::execute(
            &state_oracle_precompile,
            &request.caller,
            callback_input.as_ref(),
            request.callback_gas_limit,
            max_fee_per_gas,
            max_priority_fee_per_gas,
        )
    }
}
impl<T: Config> EthereumStateOracle for Module<T> {
    type Address = EthAddress;
    type RequestId = RequestId;
    #[doc = " Create a new remote call request"]
    #[doc = " `caller` - should be `msg.sender` and will pay for the callback fulfillment"]
    #[doc = " `bounty` - CPAY amount as incentive for relayer to fulfil the job"]
    fn new_request(
        caller: &Self::Address,
        destination: &Self::Address,
        input_data: &[u8],
        callback_signature: &[u8; 4],
        callback_gas_limit: u64,
        fee_preferences: Option<FeePreferences>,
        bounty: Balance,
    ) -> Self::RequestId {
        let request_id = NextRequestId::get();
        let request_info = CallRequest {
            caller: *caller,
            destination: *destination,
            callback_signature: *callback_signature,
            callback_gas_limit,
            fee_preferences,
            bounty,
            timestamp: T::UnixTime::now().as_secs(),
        };
        Requests::insert(request_id, request_info);
        RequestInputData::insert(request_id, input_data.to_vec());
        NextRequestId::mutate(|i| *i += U256::from(1));
        request_id
    }
}
