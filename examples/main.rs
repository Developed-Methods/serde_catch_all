use serde_json::{from_str, to_string};

use serde_catch_all::serde_catch_all;

#[serde_catch_all]
#[derive(Debug, PartialEq, Eq)]
enum Example {
    OptionA,
    #[serde(rename = "b")]
    OptionB,
    #[serde(alias = "alt1", alias = "alt2")]
    OptionC,
    #[catch_all]
    Other(String),
}

fn main() {
    // Test known variants
    assert_eq!(
        from_str::<Example>(r#""OptionA""#).unwrap(),
        Example::OptionA
    );

    // Test rename attribute
    assert_eq!(from_str::<Example>(r#""b""#).unwrap(), Example::OptionB);

    // Test original name for variant with aliases
    assert_eq!(
        from_str::<Example>(r#""OptionC""#).unwrap(),
        Example::OptionC
    );

    // Test alias attributes
    assert_eq!(from_str::<Example>(r#""alt1""#).unwrap(), Example::OptionC);
    assert_eq!(from_str::<Example>(r#""alt2""#).unwrap(), Example::OptionC);

    // Test unknown strings -> catch all
    assert_eq!(
        from_str::<Example>(r#""UnknownVariant""#).unwrap(),
        Example::Other("UnknownVariant".into())
    );

    // Test serialization
    assert_eq!(to_string(&Example::OptionA).unwrap(), r#""OptionA""#);
    assert_eq!(to_string(&Example::OptionB).unwrap(), r#""b""#);
    assert_eq!(to_string(&Example::OptionC).unwrap(), r#""OptionC""#);
    assert_eq!(
        to_string(&Example::Other("custom".into())).unwrap(),
        r#""custom""#
    );

    println!("All tests passed! The proc macro is working correctly.");
}
