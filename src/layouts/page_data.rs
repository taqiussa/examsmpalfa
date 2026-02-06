use crate::layouts::sidebar::LayoutContext;
use serde::Serialize;

#[derive(Serialize)]
pub struct PageData<T: Serialize> {
    pub title: String,
    pub layout: LayoutContext,
    pub data: T,
}
