use super::{config_genesis, get_account_id_from_seed, get_authority_keys_from_seed, ChainSpec, NetworkKeys};

fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed("Andrea"),
		get_account_id_from_seed("Brooke"),
		get_account_id_from_seed("Courtney"),
		get_account_id_from_seed("Drew"),
		get_account_id_from_seed("Emily"),
		get_account_id_from_seed("Frank"),
		get_account_id_from_seed("Centrality"),
		get_account_id_from_seed("Kauri"),
		get_account_id_from_seed("Rimu"),
		get_account_id_from_seed("cennznet-js-test"),
		get_account_id_from_seed("Andrea//stash"),
		get_account_id_from_seed("Brooke//stash"),
		get_account_id_from_seed("Courtney//stash"),
		get_account_id_from_seed("Drew//stash"),
		get_account_id_from_seed("Emily//stash"),
		get_account_id_from_seed("Frank//stash"),
		get_account_id_from_seed("Centrality//stash"),
		get_account_id_from_seed("Kauri//stash"),
		get_account_id_from_seed("Rimu//stash"),
		get_account_id_from_seed("cennznet-js-test//stash"),
	];
	let initial_authorities = vec![
		get_authority_keys_from_seed("Andrea"),
		get_authority_keys_from_seed("Brooke"),
		get_authority_keys_from_seed("Courtney"),
		get_authority_keys_from_seed("Drew"),
	];
	let root_key = get_account_id_from_seed("Kauri");

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

pub fn config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Kauri CENNZnet",
		"Kauri",
		|| config_genesis(network_keys(), false),
		vec![],
		None,
		Some("kauri"),
		None,
		Default::default(),
	)
}
