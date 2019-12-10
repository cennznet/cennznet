use genesis_config::{get_authority_keys_from_seed, get_account_id_from_seed, config_genesis, GenesisConfig, NetworkKeys};

/// kauri genesis config
fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		get_account_id_from_seed("Alice"),
		get_account_id_from_seed("Bob"),
		get_account_id_from_seed("Charlie"),
		get_account_id_from_seed("Dave"),
		get_account_id_from_seed("Eve"),
		get_account_id_from_seed("Ferdie"),
		get_account_id_from_seed("Kauri"),
		get_account_id_from_seed("Rimu"),
		get_account_id_from_seed("Alice//stash"),
		get_account_id_from_seed("Bob//stash"),
		get_account_id_from_seed("Charlie//stash"),
		get_account_id_from_seed("Dave//stash"),
		get_account_id_from_seed("Eve//stash"),
		get_account_id_from_seed("Ferdie//stash"),
	];
	let initial_authorities = vec![
        get_authority_keys_from_seed("Alice"),
	];
	let root_key = get_account_id_from_seed("Alice"),

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

/// dev genesis config (single validator Alice)
fn dev_config_genesis() -> GenesisConfig {
	config_genesis(network_keys(), true)
}
