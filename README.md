# serde_catch_all

**Notice:** made with AI prompting and I didn't review too closely. It's a proc macro so shouldn't affect runtime and doesn't appear to be adding in a backdoor :).

A proc macro for creating Serde-compatible enums with catch-all variants that capture unknown string values instead of failing deserialization.

## Features

- **Catch-all variants**: Unknown string values are captured instead of causing deserialization errors
- **Serde attribute support**: Full support for `#[serde(rename = "...")]` and `#[serde(alias = "...")]`

## Usage

Add this to your `Cargo.toml`:

```toml
[dependencies]
serde_catch_all = "0.1.0"
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"  # or your preferred serde format
```

### Basic Example

```rust
use serde_catch_all::serde_catch_all;
use serde_json::{from_str, to_string};

#[serde_catch_all]
#[derive(Debug, PartialEq, Eq)]
enum Status {
    Active,
    Inactive,
    #[serde(rename = "temp-disabled")]
    TemporaryDisabled,
    #[catch_all]
    Unknown(String),
}

fn main() {
    // Known variants deserialize normally
    assert_eq!(from_str::<Status>(r#""Active""#).unwrap(), Status::Active);
    assert_eq!(from_str::<Status>(r#""temp-disabled""#).unwrap(), Status::TemporaryDisabled);

    // Unknown variants are caught instead of failing
    assert_eq!(from_str::<Status>(r#""deprecated""#).unwrap(), Status::Unknown("deprecated".to_string()));
    assert_eq!(from_str::<Status>(r#""beta""#).unwrap(), Status::Unknown("beta".to_string()));

    // Serialization works correctly
    assert_eq!(to_string(&Status::Active).unwrap(), r#""Active""#);
    assert_eq!(to_string(&Status::TemporaryDisabled).unwrap(), r#""temp-disabled""#);
    assert_eq!(to_string(&Status::Unknown("custom".to_string())).unwrap(), r#""custom""#);
}
```

## License

MIT

