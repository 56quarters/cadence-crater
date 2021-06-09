// cadence-crater - backwards compatibility testing for cadence
//
// Copyright 2021 Nick Pillitteri
//
// Licensed under the Apache License, Version 2.0 <LICENSE-APACHE or
// http://www.apache.org/licenses/LICENSE-2.0> or the MIT license
// <LICENSE-MIT or http://opensource.org/licenses/MIT>, at your
// option. This file may not be copied, modified, or distributed
// except according to those terms.

use std::error::Error;
use std::fmt;

#[derive(Debug)]
pub struct CraterError {
    msg: String,
    cause: Option<Box<dyn Error>>,
}

impl CraterError {
    pub fn new<S: Into<String>>(msg: S) -> Self {
        CraterError {
            msg: msg.into(),
            cause: None,
        }
    }

    pub fn new_err<S>(msg: S, cause: impl Error + 'static) -> Self
    where
        S: Into<String>,
    {
        CraterError {
            msg: msg.into(),
            cause: Some(Box::new(cause)),
        }
    }
}

impl fmt::Display for CraterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref err) = self.cause {
            write!(f, "{}: {}", self.msg, err)
        } else {
            write!(f, "{}", self.msg)
        }
    }
}

impl Error for CraterError {
    fn source(&self) -> Option<&(dyn Error + 'static)> {
        if let Some(ref err) = self.cause {
            Some(err.as_ref())
        } else {
            None
        }
    }
}
