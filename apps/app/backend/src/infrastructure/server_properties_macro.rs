//! Server properties macro - Generates boilerplate for server.properties.

#[macro_export]
macro_rules! generate_server_properties {
    ($( ($name:ident, $key:expr, $ty:ty, $desc:expr) ),* $(,)?) => {
        pub struct ServerProperty {
            pub name: &'static str,
            pub key: &'static str,
            pub description: &'static str,
        }

        $(
            pub struct $name;

            impl $name {
                pub fn definition() -> ServerProperty {
                    ServerProperty {
                        name: stringify!($name),
                        key: $key,
                        description: $desc,
                    }
                }

                pub fn parse(value: &str) -> Result<$ty, String> {
                    value.parse().map_err(|e| format!("Failed to parse {}: {}", $key, e))
                }

                pub fn to_string(value: &$ty) -> String {
                    value.to_string()
                }
            }
        )*

        pub const ALL_PROPERTIES: &[ServerProperty] = &[
            $(ServerProperty {
                name: stringify!($name),
                key: $key,
                description: $desc,
            }),*
        ];
    };
}
