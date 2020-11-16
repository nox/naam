use proc_macro::TokenStream;
use syn::DeriveInput;
use synstructure::{MacroResult, Structure};

mod dump;

/// Derive macro for the `Dump<'tape>` trait
///
/// This macro *always* derive the trait for the `'tape` lifetime, so be sure
/// to use the same name `'tape` if your operations contain fields of types such
/// as `Offset<'tape>`.
///
/// If the type for which the implementation is derived has no type parameters
/// nor a lifetime parameter named `'tape`, the generated code just delegates
/// to the `Debug` impl of that type.
///
/// Otherwise, code using `core::fmt::Formatter::debug_*` methods is generated,
/// passing each field to a `field` method on the `Debug*` value just like
/// `#[derive(Debug)]` does, except that fields whose types include `'tape` or
/// a type parameter are passed as `&naam::debug_info::Dumper::debug(&self.foo)`
/// instead of `&self.foo`.
#[proc_macro_derive(Dump)]
pub fn dump_derive(tokens: TokenStream) -> TokenStream {
    match syn::parse::<DeriveInput>(tokens) {
        Ok(p) => match Structure::try_new(&p) {
            Ok(s) => MacroResult::into_stream(dump::derive(s)),
            Err(e) => e.to_compile_error().into(),
        },
        Err(e) => e.to_compile_error().into(),
    }
}
