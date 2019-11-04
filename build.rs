// Copyright 2019 Centrality Investments Limited
//
// Licensed under the Apache License, Version 2.0 (the "License");
// you may not use this file except in compliance with the License.
// You may obtain a copy of the License at
//
//     http://www.apache.org/licenses/LICENSE-2.0
//
// Unless required by applicable law or agreed to in writing, software
// distributed under the License is distributed on an "AS IS" BASIS,
// WITHOUT WARRANTIES OR CONDITIONS OF ANY KIND, either express or implied.
// See the License for the specific language governing permissions and
// limitations under the License.

use std::{env, path::PathBuf};
use vergen::{generate_cargo_keys, ConstantsFlags};

const ERROR_MSG: &str = "Failed to generate metadata files";

fn main() {
	generate_cargo_keys(ConstantsFlags::SHA_SHORT).expect(ERROR_MSG);

	let mut manifest_dir =
		PathBuf::from(env::var("CARGO_MANIFEST_DIR").expect("`CARGO_MANIFEST_DIR` is always set by cargo."));

	while manifest_dir.parent().is_some() {
		if manifest_dir.join(".git/HEAD").exists() {
			println!("cargo:rerun-if-changed={}", manifest_dir.join(".git/HEAD").display());
			return;
		}

		manifest_dir.pop();
	}

	println!("cargo:warning=Could not find `.git/HEAD` from manifest dir!");
}
