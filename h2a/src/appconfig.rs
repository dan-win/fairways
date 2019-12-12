

use actix_web::{web, http};

use actix_web::get;  // <- here is "decorator"

use actix_cors::Cors;

use crate::actors::{by_get, by_post};


// fn amqp_pub_post((route, conf, query, state): (web::Path<String>, web::Json<models::LinkConf>, Query<WriteModeQuery>, web::Data<AppState>)) -> impl Future<Item = HttpResponse, Error = Error> {
//     // let msg = messages::CreateRoute {
//     //     route: route.to_string(),
//     //     conf: conf.into_inner(),
//     //     force: query.force
//     // };
//     // state.router
//     //     .send(msg)
//     //     .from_err()
//     //     .and_then(|ok| {
//     //         if ok {
//     //             Ok(HttpResponse::Ok().json(OperationResponse::default()))
//     //         } else {
//     //             Ok(actix_web::HttpResponse::Conflict().content_type("text/plain").body("Resource already exists"))
//     //         }
//     //     })
//     Ok(HttpResponse::Ok().body(""))
// }



#[get("/robots.txt")]
fn robots_txt() -> &'static str {
    "User-agent: *\nDisallow: /\r\n"
}

pub fn cors_middleware() -> Cors {

    let X_COOKIE_HEADER: &'static str = "x-cookie";

    // Cors::default()
    Cors::new()
        // .allowed_origin("*") // <- Avoid credentials in client!!! See: https://developer.mozilla.org/en-US/docs/Web/HTTP/Headers/Access-Control-Allow-Origin
        .allowed_methods(vec![
            "HEAD",
            "OPTIONS",
            "GET", 
            "POST",
            "DELETE"
            ])
        .allowed_headers(vec![
            http::header::ACCEPT,
            http::header::CONTENT_TYPE,
            http::header::CONTENT_LENGTH,
            // http::header::USER_AGENT,
            http::header::ORIGIN, 
            // http::header::AUTHORIZATION, 
            http::header::COOKIE, 
            http::header::HeaderName::from_static(X_COOKIE_HEADER),
            ])
        .supports_credentials()
        .max_age(3600)        
}


pub fn config_app(cfg: &mut web::ServiceConfig) {
    // domain includes: /products/{product_id}/parts/{part_id}
    cfg.service(
        web::scope("/")
            .service(web::resource("/send/{tail:.*}")
                .route(web::get().to_async(by_get))
                .route(web::post().to_async(by_post)))
            .service(robots_txt)
    );
}