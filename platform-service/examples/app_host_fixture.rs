//! Test harness helper; refuses to seed a nonempty platform. Not shipped in OS images.
#[path = "../tests/support/app_fixture.rs"]
mod fixture;
fn main() {
    let args: Vec<_> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("seed") if args.len() == 4 => {
            fixture::seed(std::path::Path::new(&args[2]), &args[3]);
        }
        Some("revoke") if args.len() == 3 => fixture::revoke(std::path::Path::new(&args[2])),
        _ => panic!(
            "usage: app_host_fixture seed STATE USER | revoke STATE (disposable test state only)"
        ),
    }
}
