use std::{boxed, error};

pub type Box = boxed::Box<dyn error::Error + Send + Sync>;
