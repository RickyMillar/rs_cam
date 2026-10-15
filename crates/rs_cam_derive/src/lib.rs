//! Derive macros for the operation parameter schema of `rs_cam_core` (W5).
//!
//! - `#[derive(OpParams)]` on an operation config struct writes the
//!   `OpParams` impl: the `ParamDef` list that the registry row publishes,
//!   the driver-set fields, the skipped fields and the field list.
//! - `#[derive(ParamEnum)]` on a parameter enum writes the `ParamType` impl:
//!   the `enum:a|b` type string, from the variant names and the serde rename
//!   rule.
//!
//! The generated code names `crate::compute::catalog::…`. So the derives
//! work only inside `rs_cam_core`, which is the only user.
//!
//! An error is a `syn::Error` that becomes a `compile_error!` at the span of
//! the bad attribute. The macros do not panic.
//!
//! # Field attributes of `OpParams`
//!
//! `#[param(...)]` takes these keys, separated by commas:
//!
//! - `help = <expr>`: the help line (a string literal or a `&str` const).
//! - `range = a..=b` or `range = a..`: an inclusive domain.
//! - `min = a`, `gt = a`, `max = b`: one bound each (`gt` is a strict
//!   lower bound). Do not use `range` together with these.
//! - `optional`: publish the field as optional when its type is not an
//!   `Option`.
//! - `frame_dependent`: the field is an absolute coordinate in the setup
//!   frame (an absolute Z). Its valid values depend on the frame, so it
//!   takes no static range; the derive refuses `range`, `min`, `gt` or
//!   `max` beside it.
//! - `skip = <reason>`: the field is not a parameter.
//! - `driver_set = <reason>`: the field is written by its own driver, and
//!   the setter refuses it with the reason.
//!
//! A field without `skip` or `driver_set` is a published parameter. Its type
//! must implement `ParamType`; the compiler refuses a field of another type.

use proc_macro::TokenStream;
use proc_macro2::{Span, TokenStream as TokenStream2};
use quote::quote;
use syn::punctuated::Punctuated;
use syn::spanned::Spanned;
use syn::{Data, DeriveInput, Expr, ExprLit, Fields, Lit, LitStr, Meta, Token};

/// Derive the `OpParams` impl of an operation config struct.
#[proc_macro_derive(OpParams, attributes(param))]
pub fn derive_op_params(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    op_params(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

/// Derive the `ParamType` impl of a parameter enum.
#[proc_macro_derive(ParamEnum)]
pub fn derive_param_enum(input: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(input as DeriveInput);
    param_enum(&input)
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

// ── OpParams ────────────────────────────────────────────────────────────

/// What one field's `#[param(...)]` attributes say.
#[derive(Default)]
struct FieldSpec {
    help: Option<Expr>,
    range: Option<Expr>,
    min: Option<Expr>,
    gt: Option<Expr>,
    max: Option<Expr>,
    optional: bool,
    frame_dependent: bool,
    skip: Option<Expr>,
    driver_set: Option<Expr>,
}

impl FieldSpec {
    fn has_param_keys(&self) -> bool {
        self.help.is_some()
            || self.range.is_some()
            || self.min.is_some()
            || self.gt.is_some()
            || self.max.is_some()
            || self.optional
            || self.frame_dependent
    }
}

/// Store `value` in `slot`, or refuse a key given twice.
fn set_once(slot: &mut Option<Expr>, value: Expr, key: &str, span: Span) -> syn::Result<()> {
    if slot.is_some() {
        return Err(syn::Error::new(
            span,
            format!("`{key}` is given twice in #[param(...)]"),
        ));
    }
    *slot = Some(value);
    Ok(())
}

fn field_spec(field: &syn::Field) -> syn::Result<FieldSpec> {
    let mut spec = FieldSpec::default();
    for attr in field.attrs.iter().filter(|a| a.path().is_ident("param")) {
        attr.parse_nested_meta(|meta| {
            let span = meta.path.span();
            let key = meta
                .path
                .get_ident()
                .map(ToString::to_string)
                .unwrap_or_default();
            match key.as_str() {
                "optional" => {
                    spec.optional = true;
                    Ok(())
                }
                "frame_dependent" => {
                    spec.frame_dependent = true;
                    Ok(())
                }
                "help" => set_once(&mut spec.help, meta.value()?.parse()?, &key, span),
                "range" => set_once(&mut spec.range, meta.value()?.parse()?, &key, span),
                "min" => set_once(&mut spec.min, meta.value()?.parse()?, &key, span),
                "gt" => set_once(&mut spec.gt, meta.value()?.parse()?, &key, span),
                "max" => set_once(&mut spec.max, meta.value()?.parse()?, &key, span),
                "skip" => set_once(&mut spec.skip, meta.value()?.parse()?, &key, span),
                "driver_set" => set_once(&mut spec.driver_set, meta.value()?.parse()?, &key, span),
                _ => Err(meta.error(
                    "unknown #[param] key; expected help, range, min, gt, max, \
                     optional, frame_dependent, skip or driver_set",
                )),
            }
        })?;
    }
    if spec.skip.is_some() && spec.driver_set.is_some() {
        return Err(syn::Error::new(
            field.span(),
            "a field is `skip` or `driver_set`, not both",
        ));
    }
    if (spec.skip.is_some() || spec.driver_set.is_some()) && spec.has_param_keys() {
        return Err(syn::Error::new(
            field.span(),
            "a `skip` or `driver_set` field is not published, so it takes no help, \
             range or optional",
        ));
    }
    if spec.range.is_some() && (spec.min.is_some() || spec.gt.is_some() || spec.max.is_some()) {
        return Err(syn::Error::new(
            field.span(),
            "give `range` alone, or the bounds `min`/`gt`/`max`, not both",
        ));
    }
    if spec.frame_dependent
        && (spec.range.is_some() || spec.min.is_some() || spec.gt.is_some() || spec.max.is_some())
    {
        return Err(syn::Error::new(
            field.span(),
            "a `frame_dependent` field is an absolute coordinate in the setup frame, so it \
             takes no static range",
        ));
    }
    if spec.min.is_some() && spec.gt.is_some() {
        return Err(syn::Error::new(
            field.span(),
            "give one lower bound: `min` (inclusive) or `gt` (strict)",
        ));
    }
    Ok(spec)
}

/// The `Option<ParamRange>` expression of one field.
fn range_tokens(spec: &FieldSpec, catalog: &TokenStream2) -> syn::Result<TokenStream2> {
    let some = |e: &Expr| quote!(::core::option::Option::Some(#e));
    let none = quote!(::core::option::Option::None);
    let (min, min_exclusive, max) = if let Some(range) = &spec.range {
        let Expr::Range(r) = range else {
            return Err(syn::Error::new(
                range.span(),
                "`range` takes `a..=b` or `a..`",
            ));
        };
        let end = match (&r.limits, &r.end) {
            (syn::RangeLimits::Closed(_), Some(end)) => some(end),
            (syn::RangeLimits::HalfOpen(_), None) => none.clone(),
            _ => {
                return Err(syn::Error::new(
                    range.span(),
                    "`range` takes `a..=b` (inclusive) or `a..` (no upper bound)",
                ));
            }
        };
        let Some(start) = &r.start else {
            return Err(syn::Error::new(
                range.span(),
                "`range` needs a lower bound; use `max = b` for an upper bound alone",
            ));
        };
        (some(start), false, end)
    } else {
        let (min, exclusive) = match (&spec.min, &spec.gt) {
            (Some(m), None) => (some(m), false),
            (None, Some(g)) => (some(g), true),
            _ => (none.clone(), false),
        };
        let max = spec.max.as_ref().map_or_else(|| none.clone(), some);
        (min, exclusive, max)
    };
    if spec.range.is_none() && spec.min.is_none() && spec.gt.is_none() && spec.max.is_none() {
        return Ok(none);
    }
    Ok(quote! {
        ::core::option::Option::Some(#catalog::ParamRange {
            min: #min,
            max: #max,
            min_exclusive: #min_exclusive,
        })
    })
}

fn op_params(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Struct(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "OpParams derives only on a struct with named fields",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(syn::Error::new(
            input.ident.span(),
            "OpParams derives only on a struct with named fields",
        ));
    };
    let catalog = quote!(crate::compute::catalog);
    let mut defs = Vec::new();
    let mut driver_set = Vec::new();
    let mut skipped = Vec::new();
    let mut names = Vec::new();
    for field in &fields.named {
        let Some(ident) = &field.ident else {
            return Err(syn::Error::new(field.span(), "a field needs a name"));
        };
        let name = ident.to_string();
        let name = name.strip_prefix("r#").unwrap_or(&name).to_owned();
        let spec = field_spec(field)?;
        names.push(name.clone());
        if let Some(reason) = &spec.driver_set {
            driver_set.push(quote! {
                #catalog::DriverSetField { name: #name, reason: #reason }
            });
            continue;
        }
        if let Some(reason) = &spec.skip {
            skipped.push(quote! {
                #catalog::SkippedField { name: #name, reason: #reason }
            });
            continue;
        }
        let ty = &field.ty;
        let help = spec.help.as_ref().map_or_else(
            || quote!(::core::option::Option::None),
            |h| quote!(::core::option::Option::Some(#h)),
        );
        let range = range_tokens(&spec, &catalog)?;
        let force_optional = spec.optional;
        let frame_dependent = spec.frame_dependent;
        defs.push(quote! {
            #catalog::ParamDef {
                name: #name,
                type_name: <#ty as #catalog::ParamType>::TYPE_NAME,
                optional: <#ty as #catalog::ParamType>::OPTIONAL || #force_optional,
                help: #help,
                range: #range,
                frame_dependent: #frame_dependent,
            }
        });
    }
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics #catalog::OpParams for #ident #ty_generics #where_clause {
            const PARAMS: #catalog::ParamTable = #catalog::ParamTable {
                defs: &[#(#defs),*],
                driver_set: &[#(#driver_set),*],
                skipped: &[#(#skipped),*],
                fields: &[#(#names),*],
            };
        }
    })
}

// ── ParamEnum ───────────────────────────────────────────────────────────

/// The serde `rename_all` rules this derive knows.
#[derive(Clone, Copy)]
enum RenameRule {
    None,
    Lower,
    Upper,
    Pascal,
    Camel,
    Snake,
    ScreamingSnake,
    Kebab,
    ScreamingKebab,
}

impl RenameRule {
    fn parse(lit: &LitStr) -> syn::Result<Self> {
        Ok(match lit.value().as_str() {
            "lowercase" => Self::Lower,
            "UPPERCASE" => Self::Upper,
            "PascalCase" => Self::Pascal,
            "camelCase" => Self::Camel,
            "snake_case" => Self::Snake,
            "SCREAMING_SNAKE_CASE" => Self::ScreamingSnake,
            "kebab-case" => Self::Kebab,
            "SCREAMING-KEBAB-CASE" => Self::ScreamingKebab,
            other => {
                return Err(syn::Error::new(
                    lit.span(),
                    format!("ParamEnum does not know the serde rename rule `{other}`"),
                ));
            }
        })
    }

    /// The serialized name of a variant, by serde's own rule.
    fn apply(self, variant: &str) -> String {
        let snake = || {
            let mut out = String::new();
            for (i, ch) in variant.char_indices() {
                if i > 0 && ch.is_uppercase() {
                    out.push('_');
                }
                out.push(ch.to_ascii_lowercase());
            }
            out
        };
        match self {
            Self::None | Self::Pascal => variant.to_owned(),
            Self::Lower => variant.to_ascii_lowercase(),
            Self::Upper => variant.to_ascii_uppercase(),
            Self::Camel => {
                let mut chars = variant.chars();
                chars.next().map_or_else(String::new, |first| {
                    first.to_ascii_lowercase().to_string() + chars.as_str()
                })
            }
            Self::Snake => snake(),
            Self::ScreamingSnake => snake().to_ascii_uppercase(),
            Self::Kebab => snake().replace('_', "-"),
            Self::ScreamingKebab => snake().replace('_', "-").to_ascii_uppercase(),
        }
    }
}

/// The `key = "value"` and bare `key` items of every `#[serde(...)]`.
fn serde_items(attrs: &[syn::Attribute]) -> syn::Result<Vec<Meta>> {
    let mut items = Vec::new();
    for attr in attrs.iter().filter(|a| a.path().is_ident("serde")) {
        let list = attr.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        items.extend(list);
    }
    Ok(items)
}

/// The string literal of a `key = "value"` item.
fn str_value(meta: &Meta) -> syn::Result<&LitStr> {
    if let Meta::NameValue(nv) = meta
        && let Expr::Lit(ExprLit {
            lit: Lit::Str(s), ..
        }) = &nv.value
    {
        return Ok(s);
    }
    Err(syn::Error::new(
        meta.span(),
        "ParamEnum reads this serde key only in the form `key = \"value\"`",
    ))
}

fn param_enum(input: &DeriveInput) -> syn::Result<TokenStream2> {
    let Data::Enum(data) = &input.data else {
        return Err(syn::Error::new(
            input.ident.span(),
            "ParamEnum derives only on an enum",
        ));
    };
    let mut rule = RenameRule::None;
    for item in serde_items(&input.attrs)? {
        if item.path().is_ident("rename_all") {
            rule = RenameRule::parse(str_value(&item)?)?;
        }
    }
    let mut tokens = Vec::new();
    for variant in &data.variants {
        if !matches!(variant.fields, Fields::Unit) {
            return Err(syn::Error::new(
                variant.span(),
                "ParamEnum takes unit variants only",
            ));
        }
        let mut name = rule.apply(&variant.ident.to_string());
        let mut skipped = false;
        for item in serde_items(&variant.attrs)? {
            let path = item.path();
            if path.is_ident("rename") {
                name = str_value(&item)?.value();
            } else if path.is_ident("skip") || path.is_ident("skip_deserializing") {
                skipped = true;
            }
        }
        if !skipped {
            tokens.push(name);
        }
    }
    if tokens.is_empty() {
        return Err(syn::Error::new(
            input.ident.span(),
            "ParamEnum found no variant that serde reads",
        ));
    }
    let type_name = format!("enum:{}", tokens.join("|"));
    let ident = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();
    Ok(quote! {
        impl #impl_generics crate::compute::catalog::ParamType for #ident #ty_generics #where_clause {
            const TYPE_NAME: &'static str = #type_name;
            const OPTION_TYPE_NAME: &'static str = #type_name;
        }
    })
}
