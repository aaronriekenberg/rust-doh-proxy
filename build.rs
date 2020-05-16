use vergen::{generate_cargo_keys, ConstantsFlags};

fn main() {
    let mut flags = ConstantsFlags::empty();
    flags.toggle(ConstantsFlags::BUILD_TIMESTAMP);
    flags.toggle(ConstantsFlags::SEMVER_LIGHTWEIGHT);
    flags.toggle(ConstantsFlags::SHA);
    flags.toggle(ConstantsFlags::TARGET_TRIPLE);

    generate_cargo_keys(flags).expect("Unable to generate the cargo keys!");
}
