/// Trait for enums that can be used as a dynamic entry point for a `banish!`
/// block via the `#![dispatch(expr)]` block attribute.
///
/// `variant_name` must return the snake_case name of the current variant as a
/// `&'static str`. This is matched against state names at runtime to select
/// the entry state. The return value is a static string so there is no
/// allocation on dispatch.
///
/// # Deriving
///
/// The easiest way to implement this trait is with `#[derive(BanishDispatch)]`,
/// which generates the correct `variant_name` implementation automatically.
/// Works on all enum variants regardless of whether they carry data.
/// The data is ignored, only the variant name is used for dispatch.
///
/// ```rust
/// use banish::BanishDispatch;
///
/// #[derive(BanishDispatch)]
/// enum PipelineState {
///     Normalize,
///     Finalize,
///     Done,
/// }
///
/// let state = PipelineState::Normalize;
/// assert_eq!(state.variant_name(), "normalize");
/// ```
///
/// # Manual Implementation
///
/// If you need custom behavior or are not using the derive macro:
///
/// ```rust
/// use banish::BanishDispatch;
///
/// enum PipelineState {
///     Normalize,
///     Finalize,
/// }
///
/// impl BanishDispatch for PipelineState {
///     fn variant_name(&self) -> &'static str {
///         match self {
///             PipelineState::Normalize => "normalize",
///             PipelineState::Finalize => "finalize",
///         }
///     }
/// }
/// ```
pub trait BanishDispatch {
    fn variant_name(&self) -> &'static str;
}