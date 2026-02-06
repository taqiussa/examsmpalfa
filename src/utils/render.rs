use axum::response::Html;
use serde::Serialize;
use tera::Context;

use crate::{
    layouts::{page_data::PageData, sidebar::LayoutContext},
    utils::{page_context::PageContext, vite},
};

pub fn render<T: Serialize>(
    ctx: &PageContext,
    template: &str,
    title: &str,
    data: T,
) -> Html<String> {
    let page = PageData {
        title: title.to_string(),
        layout: LayoutContext::with_user(&ctx.user, ctx.uri.path()),
        data,
    };

    let mut tera_ctx = Context::from_serialize(&page).unwrap();

    tera_ctx.insert("vite_css", &vite::vite_css());
    tera_ctx.insert("vite_js", &vite::vite_js());

    let rendered = ctx.tera.render(template, &tera_ctx).unwrap();

    Html(rendered)
}

#[cfg(test)]
mod tests {
    use super::render;
    use crate::{
        models::{auth_user::AuthUser, role::Role},
        utils::page_context::PageContext,
    };
    use axum::http::Uri;

    #[test]
    fn render_includes_title_and_layout() {
        let tera = crate::test_support::build_test_tera();
        let ctx = PageContext {
            user: AuthUser {
                id: 1,
                roles: vec![Role::Admin],
                name: "Admin".into(),
                foto: None,
            },
            tera,
            uri: Uri::from_static("/dashboard"),
        };

        let html = render(&ctx, "dashboard/index.html", "Dashboard", ());
        let body = html.0;
        assert!(body.contains("Dashboard"));
    }
}
