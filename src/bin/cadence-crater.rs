//

use cadence_crater::cmd::CraterApplication;
use clap::Clap;

fn main() {
    let app = CraterApplication::parse();
    let res = app.run();

    if let Err(e) = res {
        eprintln!("cadence-crater: {}", e);
    }
}
