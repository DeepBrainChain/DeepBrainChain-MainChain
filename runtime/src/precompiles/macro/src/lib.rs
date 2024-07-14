use proc_macro::TokenStream;
use proc_macro2::Literal;
use quote::quote;
use sha3::{Digest, Keccak256};
use syn::{parse_macro_input, Expr, ExprLit, Ident, ItemEnum, Lit, LitByteStr, LitStr};

#[proc_macro_attribute]
pub fn generate_function_selector(_: TokenStream, input: TokenStream) -> TokenStream {
    let item = parse_macro_input!(input as ItemEnum);

    let ItemEnum { attrs, vis, enum_token, ident, variants, .. } = item;

    let mut ident_expressions: Vec<Ident> = vec![];
    let mut variant_expressions: Vec<Expr> = vec![];
    for variant in variants {
        if let Some((_, Expr::Lit(ExprLit { lit, .. }))) = variant.discriminant {
            if let Lit::Str(token) = lit {
                let selector = get_function_selector(&token.value());
                // println!("method: {:?}, selector: {:?}", token.value(), selector);
                ident_expressions.push(variant.ident);
                variant_expressions.push(Expr::Lit(ExprLit {
                    lit: Lit::Verbatim(Literal::u32_suffixed(selector)),
                    attrs: Default::default(),
                }));
            } else {
                panic!("Not method string: `{:?}`", lit);
            }
        } else {
            panic!("Not enum: `{:?}`", variant);
        }
    }

    (quote! {
        #(#attrs)*
        #vis #enum_token #ident {
            #(
                #ident_expressions = #variant_expressions,
            )*
        }
    })
    .into()
}

#[proc_macro]
pub fn keccak256(input: TokenStream) -> TokenStream {
    let lit_str = parse_macro_input!(input as LitStr);

    let result = sha3_256(&lit_str.value());

    let eval = Lit::ByteStr(LitByteStr::new(result.as_ref(), proc_macro2::Span::call_site()));

    quote!(#eval).into()
}

fn sha3_256(s: &str) -> [u8; 32] {
    let mut result = [0u8; 32];

    // create a SHA3-256 object
    let mut hasher = Keccak256::new();
    // write input message
    hasher.update(s);
    // read hash digest
    result.copy_from_slice(&hasher.finalize()[..32]);

    result
}

fn get_function_selector(s: &str) -> u32 {
    let result = sha3_256(s);
    u32::from_be_bytes(result[..4].try_into().unwrap())
}
