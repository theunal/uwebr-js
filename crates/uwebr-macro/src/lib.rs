use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput, ItemFn};

/// `#[component]` attribute macro for functional components.
#[proc_macro_attribute]
pub fn component(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let fn_vis = &input_fn.vis;
    let fn_block = &input_fn.block;
    let fn_sig = &input_fn.sig;
    let fn_attrs = &input_fn.attrs;

    let expanded = quote! {
        #(#fn_attrs)*
        #fn_vis #fn_sig {
            let __cid = ::uwebr_core::lifecycle::create_component_scope();
            ::uwebr_core::lifecycle::with_component(__cid, || {
                ::uwebr_core::lifecycle::on_cleanup(move || {
                    ::uwebr_core::lifecycle::trigger_cleanup(__cid);
                });
                let __r = #fn_block;
                ::uwebr_core::lifecycle::trigger_mount(__cid);
                __r
            })
        }
    };

    TokenStream::from(expanded)
}

/// `#[derive(Props)]` macro — generates a builder pattern for the struct.
///
/// # Example
/// ```ignore
/// #[derive(Props)]
/// struct ButtonProps {
///     label: String,
///     disabled: bool,
/// }
///
/// // Usage:
/// let props = ButtonProps::builder()
///     .label("Click".into())
///     .disabled(false)
///     .build()?;
/// ```
#[proc_macro_derive(Props)]
pub fn derive_props(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "Props can only be derived for structs with named fields",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "Props can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let field_names: Vec<_> = fields.iter().map(|f| f.ident.as_ref().unwrap()).collect();
    let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

    let builder_methods: Vec<_> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(fname, ftype)| {
            quote! {
                pub fn #fname(mut self, value: #ftype) -> Self {
                    self.#fname = ::std::option::Option::Some(value);
                    self
                }
            }
        })
        .collect();

    let builder_fields: Vec<_> = field_names
        .iter()
        .zip(field_types.iter())
        .map(|(fname, ftype)| {
            quote! { #fname: ::std::option::Option<#ftype> }
        })
        .collect();

    let builder_new_fields: Vec<_> = field_names
        .iter()
        .map(|fname| quote! { #fname: ::std::option::Option::None })
        .collect();

    let build_fields: Vec<_> = field_names
        .iter()
        .map(|fname| {
            let fname_str = fname.to_string();
            quote! {
                #fname: self.#fname.ok_or_else(|| {
                    ::std::string::String::from(concat!("Missing required prop: ", #fname_str))
                })?
            }
        })
        .collect();

    let builder_name = syn::Ident::new(&format!("{}Builder", name), name.span());

    let expanded = quote! {
        impl #name {
            pub fn builder() -> #builder_name {
                #builder_name::new()
            }
        }

        pub struct #builder_name {
            #(#builder_fields),*,
        }

        impl #builder_name {
            fn new() -> Self {
                Self { #(#builder_new_fields),* }
            }

            #(#builder_methods)*

            pub fn build(self) -> ::std::result::Result<#name, ::std::string::String> {
                Ok(#name {
                    #(#build_fields),*
                })
            }
        }
    };

    TokenStream::from(expanded)
}
