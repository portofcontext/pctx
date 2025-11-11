pub mod logger;
pub(crate) mod prompts;
pub(crate) mod spinner;
pub mod styles;

pub(crate) static LOGO: &str = include_str!("./ascii-logo.txt");
pub(crate) static CHECK: &str = "✔";
pub(crate) static MARK: &str = "✘";
