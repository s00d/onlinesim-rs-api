//! Async/blocking parity by construction: every section method is declared once and expands into an
//! `async fn` on [`Client`](crate::Client) and a plain `fn` on `blocking::Client`, both running the same
//! [`Call`](crate::wire::Call) built by the module-level function of the same name.

macro_rules! section_methods {
    ($section:ident {
        $( $(#[$meta:meta])* fn $name:ident($($arg:ident: $ty:ty),* $(,)?) -> $ret:ty; )*
    }) => {
        #[cfg(feature = "async")]
        impl $section<'_, crate::Client> {
            $(
                $(#[$meta])*
                pub async fn $name(&self, $($arg: $ty),*) -> crate::Result<$ret> {
                    self.0.run($name($($arg),*)).await
                }
            )*
        }

        #[cfg(feature = "blocking")]
        impl $section<'_, crate::blocking::Client> {
            $(
                $(#[$meta])*
                pub fn $name(&self, $($arg: $ty),*) -> crate::Result<$ret> {
                    self.0.run($name($($arg),*))
                }
            )*
        }
    };
}
