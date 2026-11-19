//! Public free numbers: `client.free()`. No key needed.

use crate::types::FreeList;
use crate::wire::{Call, Request, decode};

/// Public free numbers. Get it with `client.free()`. No key needed.
#[derive(Debug, Clone, Copy)]
pub struct Free<'a, C>(pub(crate) &'a C);

/// Options of `free().list`; unset fields are not sent.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct FreeListOptions {
    /// Country code. Server default 44.
    pub country: Option<u32>,
    /// Number without the country code, as the keys of [`FreeList::numbers`].
    pub number: Option<String>,
    /// Page of `messages`.
    pub page: Option<u32>,
    /// SMS per page, 10–500.
    pub count: Option<u32>,
}

section_methods!(Free {
    /// Countries, the numbers of the selected country and the SMS of the selected number.
    fn list(options: &FreeListOptions) -> FreeList;
});

fn list(options: &FreeListOptions) -> Call<FreeList> {
    Call::new(
        Request::get("getFreeList")
            .opt("country", options.country)
            .opt("number", options.number.as_deref())
            .opt("page", options.page)
            .opt("count", options.count),
        |res| decode(res?),
    )
}
