// cadence-crater - backwards compatibility testing for cadence
//
// Copyright 2021 Nick Pillitteri
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use cadence_crater::cmd::CraterApplication;
use clap::Clap;

fn main() {
    let app = CraterApplication::parse();
    let res = app.run();

    if let Err(e) = res {
        eprintln!("cadence-crater: {}", e);
    }
}
