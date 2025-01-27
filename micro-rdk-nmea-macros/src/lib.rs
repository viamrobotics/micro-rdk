pub(crate) mod attributes;
pub(crate) mod composition;
pub(crate) mod utils;

use crate::composition::PgnComposition;
use proc_macro::TokenStream;

/// PgnMessageDerive is a macro that implements parsing logic for a struct in the form of a method
/// `from_bytes(data: Vec<u8>, source_id: u8) -> Result<Self, NmeaParseError>`, attribute accessors,
/// and a function `to_readings` for serializing to an instance of `GenericReadingsResult` as defined in
/// micro-RDK. Refer to the comment for `attributes::MacroAttributes` to understand the attributes
/// annotating the struct fields to customize the parsing/deserializing logic
#[proc_macro_derive(
    PgnMessageDerive,
    attributes(label, scale, lookup, bits, offset, fieldset, length_field, unit)
)]
pub fn pgn_message_derive(item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);

    match PgnComposition::from_input(&input) {
        Ok(statements) => statements.into_token_stream(&input).into(),
        Err(tokens) => tokens,
    }
}
