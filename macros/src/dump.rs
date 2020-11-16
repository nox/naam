use proc_macro2::{Span, TokenStream};
use quote::quote;
use std::borrow::Cow;
use syn::visit;
use syn::{Fields, Lifetime};
use synstructure::{BindingInfo, Structure};

pub(crate) fn derive(mut s: Structure) -> TokenStream {
    let tape_lt = s
        .ast()
        .generics
        .lifetimes()
        .find(|def| def.lifetime.ident == "tape")
        .map_or_else(
            || Cow::Owned(Lifetime::new("'tape", Span::call_site())),
            |def| Cow::Borrowed(&def.lifetime),
        );

    let (builtin_tape_lt, generated_tape_lt) = match &tape_lt {
        Cow::Owned(lt) => (None, Some(lt)),
        Cow::Borrowed(lt) => (Some(*lt), None),
    };

    let has_type_params = s.ast().generics.type_params().next().is_some();

    let body = if builtin_tape_lt.is_none() && !has_type_params {
        quote! { core::fmt::Debug::fmt(self, fmt) }
    } else {
        let mut type_classifier = TypeClassifier::new(builtin_tape_lt);

        let match_arms = s.variants().iter().map(|v| {
            let ctor = v.ast().ident.to_string();
            let pat = v.pat();
            let expr = match v.ast().fields {
                Fields::Named(..) => {
                    let unfinished = v.bindings().iter().fold(
                        quote! { core::fmt::Formatter::debug_struct(fmt, #ctor) },
                        |acc, b| {
                            let name = b.ast().ident.as_ref().unwrap().to_string();
                            let field_expr = type_classifier.field_expr(b);
                            quote! {
                                core::fmt::DebugStruct::field(
                                    &mut #acc,
                                    #name,
                                    #field_expr
                                )
                            }
                        },
                    );
                    quote! { core::fmt::DebugStruct::finish(#unfinished) }
                }
                Fields::Unnamed(..) => {
                    let unfinished = v.bindings().iter().fold(
                        quote! { core::fmt::Formatter::debug_tuple(fmt, #ctor) },
                        |acc, b| {
                            let field_expr = type_classifier.field_expr(b);
                            quote! {
                                core::fmt::DebugTuple::field(
                                    &mut #acc,
                                    #field_expr
                                )
                            }
                        },
                    );
                    quote! { core::fmt::DebugTuple::finish(#unfinished) }
                }
                Fields::Unit => quote! { fmt.write_str(#ctor) },
            };
            quote! { #pat => #expr }
        });
        quote! { match *self { #(#match_arms)* } }
    };

    s.underscore_const(true).gen_impl(quote! {
        gen impl<#generated_tape_lt> naam::debug_info::Dump<#tape_lt> for @Self {
            fn dump(
                &self,
                fmt: &mut core::fmt::Formatter,
                dumper: naam::debug_info::Dumper<#tape_lt>,
            ) -> core::fmt::Result {
                #body
            }
        }
    })
}
struct TypeClassifier<'a> {
    tape_lt: Option<&'a Lifetime>,
    should_use_debug_bridge: bool,
}

impl<'a> TypeClassifier<'a> {
    fn new(tape_lt: Option<&'a Lifetime>) -> Self {
        Self {
            tape_lt,
            should_use_debug_bridge: false,
        }
    }

    fn field_expr(&mut self, b: &BindingInfo<'_>) -> TokenStream {
        if b.referenced_ty_params().is_empty() && !self.has_tape_lifetime_param(b) {
            return quote! { #b };
        }
        quote! { &naam::debug_info::Dumper::debug(dumper, #b) }
    }

    fn has_tape_lifetime_param(&mut self, b: &BindingInfo<'_>) -> bool {
        if self.tape_lt.is_none() {
            return false;
        }
        self.should_use_debug_bridge = false;
        visit::Visit::visit_type(self, &b.ast().ty);
        self.should_use_debug_bridge
    }
}

impl<'ast> visit::Visit<'ast> for TypeClassifier<'_> {
    fn visit_lifetime(&mut self, lt: &'ast Lifetime) {
        self.should_use_debug_bridge |= self.tape_lt == Some(lt);

        visit::visit_lifetime(self, lt);
    }
}
