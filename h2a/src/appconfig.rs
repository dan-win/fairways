

use actix_web::web;

use actix_web::get;  // <- here is "decorator"

use crate::handlers::{amqp_pub};

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
            .service(robots_txt)
