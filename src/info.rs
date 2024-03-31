#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Info<T> {
    Defunct,
    Unauthorized,
    Some(T),
}

impl<T> Info<T> {
    pub fn to_option(&self) -> Option<&T> {
        match self {
            Info::Defunct | Info::Unauthorized => None,
            Info::Some(info) => Some(info),
        }
    }

    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> Info<U> {
        match self {
            Info::Defunct => Info::Defunct,
            Info::Unauthorized => Info::Unauthorized,
            Info::Some(info) => Info::Some(f(info)),
        }
    }
}

impl<T> Info<Option<T>> {
    pub fn to_inner_option(&self) -> Option<&T> {
        match self {
            Info::Defunct | Info::Unauthorized => None,
            Info::Some(info) => info.as_ref(),
        }
    }
}

impl Info<Option<String>> {
    pub fn to_str(&self) -> &str {
        match self {
            Info::Defunct => "<defunct>",
            Info::Unauthorized => "<unauthorized>",
            Info::Some(None) => "<unknown>",
            Info::Some(Some(info)) => info,
        }
    }
}

impl Info<String> {
    pub fn to_str(&self) -> &str {
        match self {
            Info::Defunct => "<defunct>",
            Info::Unauthorized => "<unauthorized>",
            Info::Some(info) => info,
        }
    }
}
