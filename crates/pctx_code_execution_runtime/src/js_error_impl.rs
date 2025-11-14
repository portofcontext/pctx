//! Shared macro for implementing `JsErrorClass` for error types

/// Macro to implement `JsErrorClass` with standard behavior
///
/// This macro provides a standard implementation of `JsErrorClass` that:
/// - Returns "Error" as the class
/// - Uses the error's Display implementation for the message
/// - Has no additional properties
/// - Returns self as the error reference
#[macro_export]
macro_rules! impl_js_error_class {
    ($error_type:ty) => {
        impl deno_error::JsErrorClass for $error_type {
            fn get_class(&self) -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Borrowed("Error")
            }

            fn get_message(&self) -> std::borrow::Cow<'static, str> {
                std::borrow::Cow::Owned(self.to_string())
            }

            fn get_additional_properties(
                &self,
            ) -> Box<dyn Iterator<Item = (std::borrow::Cow<'static, str>, deno_error::PropertyValue)>>
            {
                Box::new(std::iter::empty())
            }

            fn get_ref(&self) -> &(dyn std::error::Error + Send + Sync + 'static) {
                self
            }
        }
    };
}
