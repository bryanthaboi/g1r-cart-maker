//! The cart format core: schema, validation, the `.g1rcart` encoder and the
//! parsers the app shares with `cartkit.py`. Nothing here touches the network.

pub mod cart;
pub mod findings;
pub mod index;
pub mod indexentry;
pub mod labelart;
pub mod labeldoc;
pub mod luaenc;
pub mod modmanifest;
pub mod optionprobe;
pub mod optionschema;
pub mod pack;
pub mod scaffold;
pub mod schema;
pub mod semver;
pub mod spec;
pub mod validate;
pub mod workflow;

pub use cart::{read_cart, write_cart, Cart, CartError};
pub use findings::{Finding, Report, Severity};
pub use pack::{bundle_bytes, bundle_name, bundle_table, packed_cart};
pub use spec::{parse_option, parse_spec, Spec};
pub use validate::schema_findings;

use std::path::Path;

/// Offline validation. `cart_dir` unlocks the label-art checks.
pub fn validate_cart(cart: &Cart, cart_dir: Option<&Path>) -> Report {
    Report {
        findings: schema_findings(cart, cart_dir),
        notes: vec![
            "pins not resolved; run online validation to check every release, file id and hash"
                .to_string(),
        ],
    }
}
