use crate::{
    abi::{self, TokenSeq, TokenType},
    private::SolTypeValue,
    Result, Word,
};
use alloc::{borrow::Cow, vec::Vec};

/// A Solidity type.
///
/// This trait is implemented by types that contain ABI encoding and decoding
/// info for Solidity types. Types may be combined to express arbitrarily
/// complex Solidity types.
///
/// These types are zero cost representations of Solidity types. They do not
/// exist at runtime. They **only** contain information about the type, they do
/// not carry any data.
///
/// # Implementer's Guide
///
/// It should not be necessary to implement this trait manually. Instead, use
/// the [`sol!`] procedural macro to parse Solidity syntax into types that
/// implement this trait.
///
/// # Examples
///
/// Basic usage:
///
/// ```
/// use alloy_sol_types::{sol_data::*, SolType};
///
/// type Uint256DynamicArray = Array<Uint<256>>;
/// assert_eq!(Uint256DynamicArray::sol_type_name(), "uint256[]");
///
/// type Erc20FunctionArgs = (Address, Uint<256>);
/// assert_eq!(Erc20FunctionArgs::sol_type_name(), "(address,uint256)");
///
/// type LargeComplexType = (FixedArray<Array<Bool>, 2>, (FixedBytes<13>, String));
/// assert_eq!(
///     LargeComplexType::sol_type_name(),
///     "(bool[][2],(bytes13,string))"
/// );
/// ```
///
/// The previous example can be entirely replicated with the [`sol!`] macro:
///
/// ```
/// use alloy_sol_types::{sol, SolType};
///
/// type Uint256DynamicArray = sol!(uint256[]);
/// assert_eq!(Uint256DynamicArray::sol_type_name(), "uint256[]");
///
/// type Erc20FunctionArgs = sol!((address, uint256));
/// assert_eq!(Erc20FunctionArgs::sol_type_name(), "(address,uint256)");
///
/// type LargeComplexType = sol!((bool[][2],(bytes13,string)));
/// assert_eq!(
///     LargeComplexType::sol_type_name(),
///     "(bool[][2],(bytes13,string))"
/// );
/// ```
///
/// For more complex usage, it's recommended to use the
/// [`SolValue`](crate::SolValue) trait for primitive types, and the `Sol*`
/// traits for other types created with [`sol!`]:
///
/// ```
/// use alloy_primitives::Address;
/// use alloy_sol_types::{sol, SolCall, SolStruct, SolValue};
///
/// sol! {
///     struct MyStruct {
///         bool a;
///         uint64 b;
///         address c;
///     }
///
///     enum MyEnum {
///         A,
///         B,
///         C,
///     }
///
///     function myFunction(MyStruct my_struct, MyEnum my_enum);
/// }
///
/// // `SolValue`
/// let my_bool = true;
/// let _ = my_bool.abi_encode();
///
/// let my_struct = MyStruct {
///     a: true,
///     b: 1,
///     c: Address::ZERO,
/// };
/// let _ = my_struct.abi_encode();
///
/// let my_enum = MyEnum::A;
/// let _ = my_enum.abi_encode();
///
/// // `SolCall`
/// let my_function_call = myFunctionCall { my_struct, my_enum };
/// let _ = my_function_call.abi_encode();
/// ```
///
/// [`sol!`]: crate::sol
pub trait SolType: Sized {
    /// The corresponding Rust type.
    type RustType: SolTypeValue<Self> + 'static;

    /// The corresponding ABI [token type](TokenType).
    ///
    /// This is the intermediate representation of the type that is used for
    /// ABI encoding and decoding.
    type TokenType<'a>: TokenType<'a>;

    /// The encoded size of the type, if known at compile time
    const ENCODED_SIZE: Option<usize> = Some(32);

    /// Whether the encoded size is dynamic.
    const DYNAMIC: bool = Self::ENCODED_SIZE.is_none();

    /// Returns the name of this type in Solidity.
    fn sol_type_name() -> Cow<'static, str>;

    /// Calculate the ABI-encoded size of the data, counting both head and tail
    /// words. For a single-word type this will always be 32.
    #[inline]
    fn abi_encoded_size<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> usize {
        rust.stv_abi_encoded_size()
    }

    /// Returns `true` if the given token can be detokenized with this type.
    fn valid_token(token: &Self::TokenType<'_>) -> bool;

    /// Returns an error if the given token cannot be detokenized with this
    /// type.
    #[inline]
    fn type_check(token: &Self::TokenType<'_>) -> Result<()> {
        if Self::valid_token(token) {
            Ok(())
        } else {
            Err(crate::Error::type_check_fail_token::<Self>(token))
        }
    }

    /// Detokenize this type's value from the given token.
    ///
    /// See the [`abi::token`] module for more information.
    fn detokenize(token: Self::TokenType<'_>) -> Self::RustType;

    /// Tokenizes the given value into this type's token.
    ///
    /// See the [`abi::token`] module for more information.
    fn tokenize<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> Self::TokenType<'_> {
        rust.stv_to_tokens()
    }

    /// Encode this data according to EIP-712 `encodeData` rules, and hash it
    /// if necessary.
    ///
    /// Implementer's note: All single-word types are encoded as their word.
    /// All multi-word types are encoded as the hash the concatenated data
    /// words for each element
    ///
    /// <https://eips.ethereum.org/EIPS/eip-712#definition-of-encodedata>
    #[inline]
    fn eip712_data_word<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> Word {
        rust.stv_eip712_data_word()
    }

    /// Non-standard Packed Mode ABI encoding.
    ///
    /// See [`abi_encode_packed`][SolType::abi_encode_packed] for more details.
    #[inline]
    fn abi_encode_packed_to<E: ?Sized + SolTypeValue<Self>>(rust: &E, out: &mut Vec<u8>) {
        rust.stv_abi_encode_packed_to(out)
    }

    /// Non-standard Packed Mode ABI encoding.
    ///
    /// This is different from normal ABI encoding:
    /// - types shorter than 32 bytes are concatenated directly, without padding
    ///   or sign extension;
    /// - dynamic types are encoded in-place and without the length;
    /// - array elements are padded, but still encoded in-place.
    ///
    /// More information can be found in the [Solidity docs](https://docs.soliditylang.org/en/latest/abi-spec.html#non-standard-packed-mode).
    #[inline]
    fn abi_encode_packed<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> Vec<u8> {
        let mut out = Vec::new();
        Self::abi_encode_packed_to(rust, &mut out);
        out
    }

    /// Tokenizes and ABI-encodes the given value by wrapping it in a
    /// single-element sequence.
    ///
    /// See the [`abi`] module for more information.
    #[inline]
    fn abi_encode<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> Vec<u8> {
        abi::encode(&rust.stv_to_tokens())
    }

    /// Tokenizes and ABI-encodes the given value as function parameters.
    ///
    /// See the [`abi`] module for more information.
    #[inline]
    fn abi_encode_params<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> Vec<u8>
    where
        for<'a> Self::TokenType<'a>: TokenSeq<'a>,
    {
        abi::encode_params(&rust.stv_to_tokens())
    }

    /// Tokenizes and ABI-encodes the given value as a sequence.
    ///
    /// See the [`abi`] module for more information.
    #[inline]
    fn abi_encode_sequence<E: ?Sized + SolTypeValue<Self>>(rust: &E) -> Vec<u8>
    where
        for<'a> Self::TokenType<'a>: TokenSeq<'a>,
    {
        abi::encode_sequence(&rust.stv_to_tokens())
    }

    /// Decodes this type's value from an ABI blob by interpreting it as a
    /// single-element sequence.
    ///
    /// See the [`abi`] module for more information.
    #[inline]
    fn abi_decode(data: &[u8], validate: bool) -> Result<Self::RustType> {
        abi::decode::<Self::TokenType<'_>>(data, validate).and_then(check_decode::<Self>(validate))
    }

    /// Decodes this type's value from an ABI blob by interpreting it as
    /// function parameters.
    ///
    /// See the [`abi`] module for more information.
    #[inline]
    fn abi_decode_params<'de>(data: &'de [u8], validate: bool) -> Result<Self::RustType>
    where
        Self::TokenType<'de>: TokenSeq<'de>,
    {
        abi::decode_params::<Self::TokenType<'_>>(data, validate)
            .and_then(check_decode::<Self>(validate))
    }

    /// Decodes this type's value from an ABI blob by interpreting it as a
    /// sequence.
    ///
    /// See the [`abi`] module for more information.
    #[inline]
    fn abi_decode_sequence<'de>(data: &'de [u8], validate: bool) -> Result<Self::RustType>
    where
        Self::TokenType<'de>: TokenSeq<'de>,
    {
        abi::decode_sequence::<Self::TokenType<'_>>(data, validate)
            .and_then(check_decode::<Self>(validate))
    }
}

#[inline]
fn check_decode<T: SolType>(
    validate: bool,
) -> impl FnOnce(T::TokenType<'_>) -> Result<T::RustType> {
    move |token| {
        if validate {
            T::type_check(&token)?;
        }
        Ok(T::detokenize(token))
    }
}
