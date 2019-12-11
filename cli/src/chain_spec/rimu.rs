use super::{config_genesis, ChainSpec, NetworkKeys};
use hex_literal::hex;
use primitives::crypto::UncheckedInto;

/// rimu genesis config
fn network_keys() -> NetworkKeys {
	let endowed_accounts = vec![
		hex!["3aebf8155bd297575b4ce00c1e620d5e1701600bda1eb70f72b547ee18c6682e"].unchecked_into(),
		hex!["c4624fe230a2183cb948224ca74aa9108903c305b6fc90a193765ddbf963b328"].unchecked_into(),
		hex!["6ad2b857ee8567bb7deca480ec93f81f854232a143140ba2f2398cf1d0d63d70"].unchecked_into(),
		hex!["1ad144301298528620c6f3c1b0543da4da03b21f06c77d1ce59bdf323f0ad750"].unchecked_into(),
		hex!["8af545920eba09438064452bd5c751217ff330de18b5788020151f96b3acd365"].unchecked_into(),
		hex!["ec4bab950ecb796669b074682600c19b4d1a4ff9ed101e0b5742bcce69f42b7f"].unchecked_into(),
		hex!["3040e1512f5672c364a459bb78dbda76fd9f3a88adc31159e56277055d478202"].unchecked_into(),
		hex!["6e471a4dce84a1c726f5a76dfb12726dde0e9816cc6a7c5ffa60d034b12b777a"].unchecked_into(),
		hex!["4a5a85ec5d121ed5fc4fa0111c060fe5184049d28f34a1856451461d9ddae341"].unchecked_into(),
		hex!["feca098c25921f5294b656dee9e05d44dba944361834ed2f17ca422696302801"].unchecked_into(),
		hex!["2c42bfb9412b21ee3dd3738b165824a7cb021d885152797d04969c3e5e9f0725"].unchecked_into(),
		hex!["442f2e719cd86309778a9eda69cb8caab48229501a19bf7647503fb074015e5f"].unchecked_into(),
		hex!["46d64e97e22ad2b41746ede75b896b73b31cbf22628c36c285571920a3893c02"].unchecked_into(),
		hex!["4438ecb26e6cf143f48449ad220248ccd5f98ab39fc279b9dabcd1c8d9067932"].unchecked_into(),
		hex!["bab8c0b3a663a84cf32aa9c12a5a2b7c8567daaf40a8765ce0abd573c0ad9e21"].unchecked_into(),
	];
	let initial_authorities = vec![
		(
			hex!["3aebf8155bd297575b4ce00c1e620d5e1701600bda1eb70f72b547ee18c6682e"].unchecked_into(),
			hex!["c4624fe230a2183cb948224ca74aa9108903c305b6fc90a193765ddbf963b328"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["6ad2b857ee8567bb7deca480ec93f81f854232a143140ba2f2398cf1d0d63d70"].unchecked_into(),
			hex!["1ad144301298528620c6f3c1b0543da4da03b21f06c77d1ce59bdf323f0ad750"].unchecked_into(),
			hex!["1ade0fc31f7e3a58cc74f02aa4cec1c0759738f4e5b8bd91d7b402cdfe2c1741"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["8af545920eba09438064452bd5c751217ff330de18b5788020151f96b3acd365"].unchecked_into(),
			hex!["ec4bab950ecb796669b074682600c19b4d1a4ff9ed101e0b5742bcce69f42b7f"].unchecked_into(),
			hex!["497804545c82571ae18a8bb1899b611f630849ea4118ea237f7064e006404cf9"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["3040e1512f5672c364a459bb78dbda76fd9f3a88adc31159e56277055d478202"].unchecked_into(),
			hex!["6e471a4dce84a1c726f5a76dfb12726dde0e9816cc6a7c5ffa60d034b12b777a"].unchecked_into(),
			hex!["456437d02aee2b2c848c9efa6af598310d5806580054999fc785c8481c09fa7f"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["4a5a85ec5d121ed5fc4fa0111c060fe5184049d28f34a1856451461d9ddae341"].unchecked_into(),
			hex!["feca098c25921f5294b656dee9e05d44dba944361834ed2f17ca422696302801"].unchecked_into(),
			hex!["8882490f8cf9b7d1fb8a6c61983112f5ffbd399430fb04039ae626fa991ed9cb"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["2c42bfb9412b21ee3dd3738b165824a7cb021d885152797d04969c3e5e9f0725"].unchecked_into(),
			hex!["442f2e719cd86309778a9eda69cb8caab48229501a19bf7647503fb074015e5f"].unchecked_into(),
			hex!["6527d5a58dd6b7c4bcc0205be47ff235350b68b455900f96eff410d07bdcd732"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
		(
			hex!["46d64e97e22ad2b41746ede75b896b73b31cbf22628c36c285571920a3893c02"].unchecked_into(),
			hex!["4438ecb26e6cf143f48449ad220248ccd5f98ab39fc279b9dabcd1c8d9067932"].unchecked_into(),
			hex!["884f147f6ccadf860e7272d24d5f4b22b4e2e33f25ca3976363199ba97a5124d"].unchecked_into(),
			// FIXME: these are fake keys
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
			hex!["49fda9ab118eeaa60e49625c185c0981c3c8f73fc48ac822415a8faf0357448c"].unchecked_into(),
		),
	];
	let root_key = hex!["bab8c0b3a663a84cf32aa9c12a5a2b7c8567daaf40a8765ce0abd573c0ad9e21"].unchecked_into();

	NetworkKeys {
		endowed_accounts,
		initial_authorities,
		root_key,
	}
}

pub fn config() -> ChainSpec {
	ChainSpec::from_genesis(
		"Rimu CENNZnet",
		"Rimu",
		|| config_genesis(network_keys(), false),
		vec![],
		None,
		Some("rimu"),
		None,
		Default::default(),
	)
}
