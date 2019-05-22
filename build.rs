// Copyright (C) 2019 Centrality Investments Limited
// This file is part of CENNZnet.
//
// This program is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// This program is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with this program.  If not, see <http://www.gnu.org/licenses/>.

extern crate vergen;

use vergen::{ConstantsFlags, Vergen};

const ERROR_MSG: &'static str = "Failed to generate metadata files";

fn main() {
	let vergen = Vergen::new(ConstantsFlags::all()).expect(ERROR_MSG);

	for (k, v) in vergen.build_info() {
		println!("cargo:rustc-env={}={}", k.name(), v);
	}

	println!("cargo:rerun-if-changed=.git/HEAD");
}
