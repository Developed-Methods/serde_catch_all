use proc_macro::TokenStream;
use quote::quote;
use syn::{
    parse_macro_input, Attribute, Data, DataEnum, DeriveInput, Expr, ExprLit, Fields, Lit, Meta,
    MetaNameValue, Path, Variant,
};

/// Attribute on enum: `#[serde_catch_all]`
/// Within the enum, mark the catch-all variant: `#[catch_all]`
/// The catch-all variant must be a tuple variant with a single `String` field.
///
/// Supports `#[serde(rename = "...")]` and `#[serde(alias = "...")]` on unit variants.
#[proc_macro_attribute]
pub fn serde_catch_all(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);

    let enum_ident = &input.ident;
    let generics = &input.generics;

    let data_enum = match &input.data {
        Data::Enum(de) => de,
        _ => {
            return syn::Error::new_spanned(
                &input,
                "#[serde_catch_all] can only be applied to enums",
            )
            .to_compile_error()
            .into();
        }
    };

    let EnumInfo {
        known_arms,
        aliases_arms,
        catch_all_variant_path,
        catch_all_binding_ty_is_string,
    } = match analyze_enum(&enum_ident, &data_enum) {
        Ok(info) => info,
        Err(e) => return e.to_compile_error().into(),
    };

    if !catch_all_binding_ty_is_string {
        return syn::Error::new_spanned(
            &catch_all_variant_path,
            "the #[catch_all] variant must be a tuple with a single `String` field",
        )
        .to_compile_error()
        .into();
    }

    // Build match arms for known names and aliases
    let known_match_arms = known_arms.iter().map(|(lit, path)| {
        quote! { #lit => ::core::result::Result::Ok(#path), }
    });

    let alias_match_arms = aliases_arms.iter().map(|(lit, path)| {
        quote! { #lit => ::core::result::Result::Ok(#path), }
    });

    // Clone iterators for reuse
    let known_match_arms_2 = known_arms.iter().map(|(lit, path)| {
        quote! { #lit => ::core::result::Result::Ok(#path), }
    });

    let alias_match_arms_2 = aliases_arms.iter().map(|(lit, path)| {
        quote! { #lit => ::core::result::Result::Ok(#path), }
    });

    // Serialize arms mirror the names (first rename if present, else ident)
    let serialize_arms = known_arms.iter().map(|(lit, path)| {
        quote! { #path => serializer.serialize_str(#lit), }
    });

    let catch_all_path = &catch_all_variant_path;

    // We implement both Deserialize and Serialize to make it round-trip.
    let (_impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    // Create a clean version of the input enum without serde and catch_all attributes
    let mut cleaned_input = input.clone();
    if let Data::Enum(ref mut data_enum) = cleaned_input.data {
        for variant in &mut data_enum.variants {
            variant
                .attrs
                .retain(|attr| !is_catch_all_attr(attr) && !attr.path().is_ident("serde"));
        }
    }

    let expanded = quote! {
        // Keep the user's enum but without problematic attributes
        #cleaned_input

        impl<'de> ::serde::Deserialize<'de> for #enum_ident #ty_generics #where_clause {
            fn deserialize<D>(deserializer: D) -> ::core::result::Result<Self, D::Error>
            where
                D: ::serde::Deserializer<'de>,
            {
                struct __Visitor;
                impl<'de> ::serde::de::Visitor<'de> for __Visitor {
                    type Value = #enum_ident;

                    fn expecting(&self, f: &mut ::core::fmt::Formatter) -> ::core::fmt::Result {
                        write!(f, "a string enum")
                    }

                    fn visit_str<E>(self, v: &str) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        match v {
                            #(#known_match_arms)*
                            #(#alias_match_arms)*
                            _ => ::core::result::Result::Ok(#catch_all_path(v.to_owned())),
                        }
                    }

                    fn visit_borrowed_str<E>(self, v: &'de str) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        self.visit_str(v)
                    }

                    fn visit_string<E>(self, v: String) -> ::core::result::Result<Self::Value, E>
                    where
                        E: ::serde::de::Error,
                    {
                        match v.as_str() {
                            #(#known_match_arms_2)*
                            #(#alias_match_arms_2)*
                            _ => ::core::result::Result::Ok(#catch_all_path(v)),
                        }
                    }
                }

                deserializer.deserialize_str(__Visitor)
            }
        }

        impl ::serde::Serialize for #enum_ident #ty_generics #where_clause {
            fn serialize<S>(&self, serializer: S) -> ::core::result::Result<S::Ok, S::Error>
            where
                S: ::serde::Serializer,
            {
                match self {
                    #(#serialize_arms)*
                    #catch_all_path(s) => serializer.serialize_str(s),
                }
            }
        }
    };

    expanded.into()
}

struct EnumInfo {
    known_arms: Vec<(String, Path)>,
    aliases_arms: Vec<(String, Path)>,
    catch_all_variant_path: Path,
    catch_all_binding_ty_is_string: bool,
}

fn analyze_enum(enum_ident: &syn::Ident, de: &DataEnum) -> syn::Result<EnumInfo> {
    let mut known_arms = Vec::<(String, Path)>::new();
    let mut aliases_arms = Vec::<(String, Path)>::new();
    let mut catch_all_path: Option<Path> = None;
    let mut catch_all_is_string = false;

    for v in &de.variants {
        let is_catch_all = v.attrs.iter().any(is_catch_all_attr);

        if is_catch_all {
            // Must be tuple variant with a single String
            match &v.fields {
                Fields::Unnamed(un) if un.unnamed.len() == 1 => {
                    let ty = &un.unnamed[0].ty;
                    catch_all_is_string = is_string_type(ty);
                }
                _ => {
                    return Err(syn::Error::new_spanned(
                        v,
                        "the #[catch_all] variant must be a tuple variant with exactly one field of type `String`",
                    ));
                }
            }

            if catch_all_path.is_some() {
                return Err(syn::Error::new_spanned(
                    v,
                    "only one #[catch_all] variant is allowed",
                ));
            }
            catch_all_path = Some(variant_path(enum_ident, v));
            continue;
        }

        // Known variants must be unit
        match &v.fields {
            Fields::Unit => { /* ok */ }
            _ => {
                return Err(syn::Error::new_spanned(
                    v,
                    "non-catch-all variants must be unit variants",
                ));
            }
        }

        // Extract names and aliases
        let (primary_name, aliases) = extract_serde_names(&v.attrs, v.ident.to_string())?;

        let path = variant_path(enum_ident, v);

        // Add primary name to known_arms
        known_arms.push((primary_name, path.clone()));

        // Add aliases to aliases_arms
        for alias in aliases {
            aliases_arms.push((alias, path.clone()));
        }
    }

    let catch_all_variant_path = catch_all_path.ok_or_else(|| {
        syn::Error::new_spanned(
            enum_ident,
            "you must provide exactly one #[catch_all] variant with a single `String` field",
        )
    })?;

    Ok(EnumInfo {
        known_arms,
        aliases_arms,
        catch_all_variant_path,
        catch_all_binding_ty_is_string: catch_all_is_string,
    })
}

fn is_catch_all_attr(a: &Attribute) -> bool {
    a.path().is_ident("catch_all")
}

fn is_string_type(ty: &syn::Type) -> bool {
    match ty {
        syn::Type::Path(tp) => {
            let last = tp.path.segments.last().map(|s| s.ident.to_string());
            matches!(last.as_deref(), Some("String"))
        }
        _ => false,
    }
}

fn variant_path(enum_ident: &syn::Ident, v: &Variant) -> Path {
    let variant_ident = &v.ident;
    syn::parse_quote! { #enum_ident :: #variant_ident }
}

// Extract serde rename/alias using syn v2 API.
// Returns (primary_name, aliases_vec)
fn extract_serde_names(
    attrs: &[Attribute],
    default_name: String,
) -> syn::Result<(String, Vec<String>)> {
    let mut primary: Option<String> = None;
    let mut aliases: Vec<String> = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("serde") {
            continue;
        }

        // Parse the attribute using syn v2 API
        match &attr.meta {
            Meta::List(list) => {
                // Parse as a list of nested meta items
                let nested = list.parse_args_with(
                    syn::punctuated::Punctuated::<Meta, syn::Token![,]>::parse_terminated,
                )?;

                for meta in nested {
                    match meta {
                        Meta::NameValue(MetaNameValue { path, value, .. })
                            if path.is_ident("rename") =>
                        {
                            if let Expr::Lit(ExprLit {
                                lit: Lit::Str(s), ..
                            }) = value
                            {
                                primary = Some(s.value());
                            }
                        }
                        Meta::NameValue(MetaNameValue { path, value, .. })
                            if path.is_ident("alias") =>
                        {
                            if let Expr::Lit(ExprLit {
                                lit: Lit::Str(s), ..
                            }) = value
                            {
                                aliases.push(s.value());
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let primary_name = primary.unwrap_or(default_name);
    Ok((primary_name, aliases))
}
