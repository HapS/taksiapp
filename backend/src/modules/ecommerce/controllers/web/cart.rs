use axum::{
    extract::State,
    response::{
        Html,
        IntoResponse,
        // Redirect,
        Response,
    },
    // Extension,
};

use crate::app_state::AppState;
// use crate::config;
use crate::middleware::global_context::ViewContext;

/// Sepet sayfası - basit HTML render, veriler API'den gelecek, bu kullanıcının my accuyount içindeki sepet sayfası yani oturum açmış kullanıcıya özel siparişlerim sayfası
pub async fn my_cart_page(
    State(state): State<AppState>,
    mut ctx: ViewContext,
    _auth: crate::middleware::auth::AuthenticatedUser,
) -> Response {
    // let config = config::get_config();

    // println!("user ID in my_cart_page: {:?}", user_id);

    // if user_id.is_none() {
    //     // Giriş yapılmamışsa giriş sayfasına yönlendir
    //     return Redirect::to("/login").into_response();
    // }

    // Template context hazırla - veriler Vue.js ile API'den çekilecek
    ctx.0.insert("title", "Sepetim");
    ctx.0.insert("current_path", &format!("/my-cart"));

    // Template render et - standart error handling ile
    match state.render_frontend_template("cart/my_cart.html", &ctx.0) {
        Ok(html) => Html(html).into_response(),
        Err(e) => {
            // Show detailed Tera error page in debug mode, otherwise return generic 500
            return crate::middleware::error_handler::handle_template_error(
                &e,
                state.config.is_debug(),
            );
        }
    }
}
