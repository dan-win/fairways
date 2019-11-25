

use actix_web::web;

use actix_web::get;  // <- here is "decorator"

use crate::handlers::{amqp_pub};


fn amqp_pub_post((route, conf, query, state): (Path<String>, Json<models::LinkConf>, Query<WriteModeQuery>, web::Data<AppState>)) -> impl Future<Item = HttpResponse, Error = Error> {
    let msg = messages::CreateRoute {
        route: route.to_string(),
        conf: conf.into_inner(),
        force: query.force
    };
    state.router
        .send(msg)
        .from_err()
        .and_then(|ok| {
            if ok {
                Ok(HttpResponse::Ok().json(OperationResponse::default()))
            } else {
                Ok(actix_web::HttpResponse::Conflict().content_type("text/plain").body("Resource already exists"))
            }
        })
}



#[get("/robots.txt")]
fn robots_txt() -> &'static str {
    "User-agent: *\nDisallow: /\r\n"
}

pub fn config_app(cfg: &mut web::ServiceConfig) {
    // domain includes: /products/{product_id}/parts/{part_id}
    cfg.service(
        web::scope("/send")
            .service(web::resource("/{tail:.*}")
                .route(web::get().to_async(amqp_pub::by_get)))
                .route(web::post().to_async(amqp_pub::by_post)))
            // .service(robots_txt)
